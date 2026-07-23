#!/usr/bin/env python3
"""Generate a Cypher script that injects a synthetic Sybil community into the
follow graph, for validating TrustRank's Sybil resistance (does a dense cluster
of fake accounts gain outsized trust despite few/no genuine incoming follows?).

Every injected node gets label `:Sybil` and property `sybil_cluster: <name>`
(in addition to `:User`, so it participates in the GDS projection like a real
account) — this makes cleanup a single query:

    MATCH (s:Sybil {sybil_cluster: "<name>"}) DETACH DELETE s;

Run the generated script with cypher-shell, e.g.:

    python3 scripts/inject_sybil_community.py --count 50 --pattern clique \\
        --infiltrate-from <real_user_id> > sybil_wave1.cypher
    docker exec -i neo4j cypher-shell -u neo4j -p <password> < sybil_wave1.cypher

Then re-run `nexusd jobs run trust-recompute` (with `[trust] report_enabled = true`
to get a CSV in report_dir) and check whether the cluster's members show up with
non-negligible trust scores.
"""

import argparse
import random
import sys

# z-base32 alphabet used by real pubky ids (opaque here - no crypto validity needed,
# trust-rank code reads ids as plain strings, not the validated PubkyId type).
Z32_ALPHABET = "ybndrfg8ejkmcpqxot1uwisza345h769"
ID_LENGTH = 52


def fake_id(rng: random.Random) -> str:
    return "".join(rng.choices(Z32_ALPHABET, k=ID_LENGTH))


def build_edges(ids: list[str], pattern: str, density: float, rng: random.Random) -> list[tuple[str, str]]:
    edges = []
    n = len(ids)
    if pattern == "clique":
        edges = [(a, b) for a in ids for b in ids if a != b]
    elif pattern == "ring":
        edges = [(ids[i], ids[(i + 1) % n]) for i in range(n)]
    elif pattern == "random":
        edges = [(a, b) for a in ids for b in ids if a != b and rng.random() < density]
    return edges


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--count", type=int, default=20, help="Number of Sybil accounts to create (default: 20)")
    parser.add_argument("--cluster", default="wave1", help="Cluster name, tags nodes for later cleanup (default: wave1)")
    parser.add_argument(
        "--pattern",
        choices=["clique", "ring", "random"],
        default="clique",
        help="Internal follow topology: clique = everyone follows everyone (classic naive attack), "
        "ring = each follows the next, random = each pair connected with --density probability",
    )
    parser.add_argument("--density", type=float, default=0.3, help="Edge probability for --pattern random (default: 0.3)")
    parser.add_argument(
        "--infiltrate-from",
        nargs="*",
        default=[],
        metavar="REAL_USER_ID",
        help="Real user ids that will each follow every Sybil node (simulates the cluster tricking "
        "genuine users into following it - this is the only path trust can actually reach the cluster through)",
    )
    parser.add_argument(
        "--target-follows",
        nargs="*",
        default=[],
        metavar="REAL_USER_ID",
        help="Real user ids that every Sybil node will follow (simulates follower-count inflation "
        "of a target account; doesn't help the Sybils' own trust score, but stress-tests any "
        "follower-count-based metric alongside TrustRank)",
    )
    parser.add_argument("--seed", type=int, default=None, help="Random seed, for reproducible generation")
    args = parser.parse_args()

    rng = random.Random(args.seed)
    ids = [fake_id(rng) for _ in range(args.count)]
    internal_edges = build_edges(ids, args.pattern, args.density, rng)

    print(f"// Sybil community injection: cluster={args.cluster!r} count={args.count} pattern={args.pattern}")
    print(f"// Cleanup: MATCH (s:Sybil {{sybil_cluster: {args.cluster!r}}}) DETACH DELETE s;")
    print()
    print("// ── Sybil nodes ──")
    for i, uid in enumerate(ids):
        print(
            f'MERGE (u:User:Sybil {{id: "{uid}"}}) '
            f'SET u.name = "SYBIL_{args.cluster}_{i}", u.bio = "", u.status = "undefined", '
            f'u.links = "[]", u.indexed_at = timestamp(), u.sybil_cluster = "{args.cluster}";'
        )

    print()
    print(f"// ── Internal follows ({args.pattern}, {len(internal_edges)} edges) ──")
    for a, b in internal_edges:
        print(
            f'MATCH (u1:User {{id: "{a}"}}), (u2:User {{id: "{b}"}}) '
            f'MERGE (u1)-[:FOLLOWS {{indexed_at: timestamp()}}]->(u2);'
        )

    if args.infiltrate_from:
        print()
        print(f"// ── Infiltration follows: real users -> every Sybil ({len(args.infiltrate_from)} x {args.count}) ──")
        for real_id in args.infiltrate_from:
            for uid in ids:
                print(
                    f'MATCH (u1:User {{id: "{real_id}"}}), (u2:User {{id: "{uid}"}}) '
                    f'MERGE (u1)-[:FOLLOWS {{indexed_at: timestamp()}}]->(u2);'
                )

    if args.target_follows:
        print()
        print(f"// ── Target inflation: every Sybil -> real target ({args.count} x {len(args.target_follows)}) ──")
        for target_id in args.target_follows:
            for uid in ids:
                print(
                    f'MATCH (u1:User {{id: "{uid}"}}), (u2:User {{id: "{target_id}"}}) '
                    f'MERGE (u1)-[:FOLLOWS {{indexed_at: timestamp()}}]->(u2);'
                )

    total_edges = len(internal_edges) + len(args.infiltrate_from) * args.count + args.count * len(args.target_follows)
    print(
        f"// {args.count} Sybil nodes, {total_edges} follow edges generated. "
        f"Cleanup: MATCH (s:Sybil {{sybil_cluster: {args.cluster!r}}}) DETACH DELETE s;",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
