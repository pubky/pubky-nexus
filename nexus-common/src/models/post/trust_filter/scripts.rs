//! The Lua behind the gate. Each script runs as one atomic step, so a write
//! and the ranking check it depends on cannot straddle a ranking publish.

use std::sync::LazyLock;

use redis::{Script, ScriptInvocation};

use super::ON_KEY;
use crate::models::user::SocialGraphStatus;

/// Lua shared by the scripts below, which all take the ranking as `KEYS[2]` and
/// the switch as `KEYS[3]`.
/// - `author_of(member)`: the author of an `author:post` member.
/// - `admitted(authors)`: for each author, whether the shared sets take their
///   posts. Everyone while the filter is off or there is no ranking, otherwise
///   the ranked authors, read with one `ZMSCORE` for the whole list.
const HELPERS: &str = r"
local function author_of(member)
    local sep = string.find(member, ':', 1, true)
    return sep and string.sub(member, 1, sep - 1) or ''
end
local function admitted(authors)
    if redis.call('EXISTS', KEYS[2], KEYS[3]) < 2 then
        local all = {}
        for i = 1, #authors do all[i] = true end
        return all
    end
    return redis.call('ZMSCORE', KEYS[2], unpack(authors))
end
";

/// Adds the score/member pairs in `ARGV` to the shared set `KEYS[1]`, keeping
/// only the members whose author is admitted. Returns how many it kept.
pub(super) static ADD: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            HELPERS,
            r"local authors = {}
              for i = 2, #ARGV, 2 do authors[#authors + 1] = author_of(ARGV[i]) end
              local ok = admitted(authors)
              local keep = {}
              for j = 1, #authors do
                  if ok[j] then
                      keep[#keep + 1] = ARGV[2 * j - 1]
                      keep[#keep + 1] = ARGV[2 * j]
                  end
              end
              if #keep > 0 then redis.call('ZADD', KEYS[1], unpack(keep)) end
              return #keep / 2",
        ]
        .concat(),
    )
});

/// Adds `ARGV[1]` to the score of `ARGV[2]` in the shared set `KEYS[1]`. A
/// member that isn't there yet is created only by an increment, and only when
/// its author is admitted: an increment cannot let a hidden post in, and a
/// decrement never leaves a negative entry behind. Returns 1 when it wrote.
pub(super) static INCR: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            HELPERS,
            r"local delta, member = tonumber(ARGV[1]), ARGV[2]
              if redis.call('ZSCORE', KEYS[1], member)
                  or (delta > 0 and admitted({author_of(member)})[1]) then
                  redis.call('ZINCRBY', KEYS[1], ARGV[1], member)
                  return 1
              end
              return 0",
        ]
        .concat(),
    )
});

/// Removes from the shared set `KEYS[1]` the members in `ARGV` whose author
/// isn't admitted by the time it runs: a hide that lost a race with a newer
/// publish, or with switching off, leaves the posts alone. Returns how many it
/// removed.
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

/// A call of the gate `script` on the shared set `key`, given the ranking and
/// the switch it checks authors against.
pub(super) fn gate_call(script: &'static Script, key: &str) -> ScriptInvocation<'static> {
    let mut call = script.key(key);
    call.key(SocialGraphStatus::ranking_key()).key(ON_KEY);
    call
}

/// One [`ADD`] call writing `entries` to the shared set `key`.
pub(super) fn add_call<'a>(
    key: &str,
    entries: impl IntoIterator<Item = (f64, &'a str)>,
) -> ScriptInvocation<'static> {
    let mut call = gate_call(&ADD, key);
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
    let mut call = gate_call(&REMOVE, key);
    for member in members {
        call.arg(member);
    }
    call
}
