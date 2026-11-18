# Architecture Decision Records

An ADR records one decision: why it was needed, what was chosen, what it costs and
what else was considered. Write one when a change adds a new data flow, graph model,
feed or API contract, or when there were real alternatives.

## Files

`docs/adr/NNNN-short-title.md`, numbered in order (`0001`, `0002`, ...). Use the next
free number.

## Sections

- **Status**: Proposed, Accepted, or Superseded by ADR NNNN, with the date.
- **Context**: the problem and the constraints at the time.
- **Decision**: what was chosen, with the contract details readers need.
- **Consequences**: what it gives, what it costs, what it adds to run.
- **Alternatives Considered**: the other options and why they were not chosen.

## Process

1. Add the ADR in the PR that makes the change, with status Proposed.
2. Set it to Accepted when the PR merges.
3. An accepted ADR is not rewritten: a new decision gets a new ADR that supersedes it,
   and the old one links to it.
