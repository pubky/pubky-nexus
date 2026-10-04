//! The Lua behind the ranked sets. Each script runs as one atomic step.

use std::sync::LazyLock;

use redis::Script;

/// Lua shared by the scripts below. Each check takes a whole page in one
/// command, so a script makes a few calls per page rather than a few per post,
/// and writes and rebuilds agree on who counts as ranked.
/// - `ranked_flags(members)`: for each `author:post`, whether its author is in
///   the ranking at `KEYS[2]`.
/// - `prune(ranked, source, members)`: removes from `ranked` the `members` no
///   longer in `source`, returning how many went.
const LUA_HELPERS: &str = r"
local function ranked_flags(members)
    local authors = {}
    for i, member in ipairs(members) do
        local sep = string.find(member, ':', 1, true)
        authors[i] = sep and string.sub(member, 1, sep - 1) or ''
    end
    if #authors == 0 then return {} end
    return redis.call('ZMSCORE', KEYS[2], unpack(authors))
end
local function prune(ranked, source, members)
    if #members == 0 then return 0 end
    local scores, gone = redis.call('ZMSCORE', source, unpack(members)), {}
    for i, member in ipairs(members) do
        if not scores[i] then gone[#gone + 1] = member end
    end
    if #gone == 0 then return 0 end
    return redis.call('ZREM', ranked, unpack(gone))
end
";

/// Writes a member to a source set and, when its author is in the ranking, to
/// the ranked copy at the same score. One atomic step, so the copy never
/// disagrees with the source.
pub(super) static ADD: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            LUA_HELPERS,
            r"redis.call('ZADD', KEYS[1], ARGV[2], ARGV[1])
              if ranked_flags({ARGV[1]})[1] then
                  redis.call('ZADD', KEYS[3], ARGV[2], ARGV[1])
              end",
        ]
        .concat(),
    )
});

/// Reconciles one `ZSCAN` page of a source set with its ranked copy: the page's
/// ranked authors' posts are written at the source's score, everyone else's
/// removed. A first call also prunes a ranked set small enough for one page of
/// posts no longer in the source, so a small set takes one call. Returns
/// `{next cursor, scanned, added, removed, pruned}`.
pub(super) static RECONCILE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            LUA_HELPERS,
            r"local added, removed, pruned = 0, 0, 0
              if ARGV[1] == '0' and redis.call('ZCARD', KEYS[3]) <= tonumber(ARGV[2]) then
                  removed = prune(KEYS[3], KEYS[1], redis.call('ZRANGE', KEYS[3], 0, -1))
                  pruned = 1
              end
              local page = redis.call('ZSCAN', KEYS[1], ARGV[1], 'COUNT', ARGV[2])
              local entries, members = page[2], {}
              for i = 1, #entries, 2 do members[#members + 1] = entries[i] end
              local flags, keep, drop = ranked_flags(members), {}, {}
              for j, member in ipairs(members) do
                  if flags[j] then
                      keep[#keep + 1] = entries[2 * j]
                      keep[#keep + 1] = member
                  else
                      drop[#drop + 1] = member
                  end
              end
              if #keep > 0 then added = redis.call('ZADD', KEYS[3], 'CH', unpack(keep)) end
              if #drop > 0 then removed = removed + redis.call('ZREM', KEYS[3], unpack(drop)) end
              return {page[1], #members, added, removed, pruned}",
        ]
        .concat(),
    )
});

/// Removes, from one `ZSCAN` page of a ranked set, the posts no longer in its
/// source. Returns `{next cursor, removed}`.
pub(super) static PRUNE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        &[
            LUA_HELPERS,
            r"local page = redis.call('ZSCAN', KEYS[1], ARGV[1], 'COUNT', ARGV[2])
              local members = {}
              for i = 1, #page[2], 2 do members[#members + 1] = page[2][i] end
              return {page[1], prune(KEYS[1], KEYS[2], members)}",
        ]
        .concat(),
    )
});
