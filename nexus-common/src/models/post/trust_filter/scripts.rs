//! The Lua behind the gate. Each script runs as one atomic step, so a write
//! and the ranking check it depends on cannot straddle a ranking publish.

use std::sync::LazyLock;

use redis::{Script, ScriptInvocation};

use crate::models::user::SocialGraphStatus;

/// Lua shared by the scripts below, which all take the ranking as `KEYS[2]`.
/// - `author_of(member)`: the author of an `author:post` member.
/// - `admitted(authors)`: for each author, whether the shared sets take their
///   posts. Everyone while there is no ranking, otherwise the ranked authors,
///   read with one `ZMSCORE` for the whole list.
const HELPERS: &str = r"
local function author_of(member)
    local sep = string.find(member, ':', 1, true)
    return sep and string.sub(member, 1, sep - 1) or ''
end
local function admitted(authors)
    if redis.call('EXISTS', KEYS[2]) == 0 then
        local all = {}
        for i = 1, #authors do all[i] = true end
        return all
    end
    return redis.call('ZMSCORE', KEYS[2], unpack(authors))
end
";

/// Adds the score/member pairs in `ARGV[2..]` to the shared set `KEYS[1]`,
/// keeping only the members whose author is admitted, or all of them when
/// `ARGV[1]` is `1`. Returns how many it kept.
pub(super) static ADD: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            HELPERS,
            r"local always, authors = ARGV[1] == '1', {}
              for i = 3, #ARGV, 2 do authors[#authors + 1] = author_of(ARGV[i]) end
              local ok = always and {} or admitted(authors)
              local keep = {}
              for j = 1, #authors do
                  if always or ok[j] then
                      keep[#keep + 1] = ARGV[2 * j]
                      keep[#keep + 1] = ARGV[2 * j + 1]
                  end
              end
              if #keep > 0 then redis.call('ZADD', KEYS[1], unpack(keep)) end
              return #keep / 2",
        ]
        .concat(),
    )
});

/// Adds `ARGV[2]` to the score of `ARGV[3]` in the shared set `KEYS[1]` for
/// the engagement of `ARGV[4]`, which only counts when that actor is admitted
/// or wrote the post. A member that isn't there yet is created only by an
/// increment, and only when its author is admitted. `ARGV[1]` = `1` admits
/// everyone. Returns 1 when it wrote.
pub(super) static INCR: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            HELPERS,
            r"local always, delta, member, actor = ARGV[1] == '1', tonumber(ARGV[2]), ARGV[3], ARGV[4]
              local author = author_of(member)
              if not always and actor ~= author and not admitted({actor})[1] then return 0 end
              if redis.call('ZSCORE', KEYS[1], member)
                  or (delta > 0 and (always or admitted({author})[1])) then
                  redis.call('ZINCRBY', KEYS[1], ARGV[2], member)
                  return 1
              end
              return 0",
        ]
        .concat(),
    )
});

/// Removes from the shared set `KEYS[1]` the members in `ARGV` whose author
/// isn't admitted by the time it runs: a hide that lost a race with a newer
/// publish leaves the posts alone. Returns how many it removed.
pub(super) static REMOVE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            HELPERS,
            r"local authors, gone = {}, {}
              for i = 1, #ARGV do authors[i] = author_of(ARGV[i]) end
              local ok = admitted(authors)
              for i = 1, #ARGV do
                  if not ok[i] then gone[#gone + 1] = ARGV[i] end
              end
              if #gone == 0 then return 0 end
              return redis.call('ZREM', KEYS[1], unpack(gone))",
        ]
        .concat(),
    )
});

/// One [`ADD`] call writing `entries` to the shared set `key`, all of them
/// when `always` is set.
pub(super) fn add_call<'a>(
    key: &str,
    always: bool,
    entries: impl IntoIterator<Item = (f64, &'a str)>,
) -> ScriptInvocation<'static> {
    let mut call = ADD.key(key);
    call.key(SocialGraphStatus::ranking_key())
        .arg(u8::from(always));
    for (score, member) in entries {
        call.arg(score).arg(member);
    }
    call
}

/// One [`REMOVE`] call taking `members` out of the shared set `key`.
pub(super) fn remove_call<'a>(
    key: &str,
    members: impl IntoIterator<Item = &'a str>,
) -> ScriptInvocation<'static> {
    let mut call = REMOVE.key(key);
    call.key(SocialGraphStatus::ranking_key());
    for member in members {
        call.arg(member);
    }
    call
}
