# Pre-submission Checklist

- [ ] I manually reviewed the PR
- [ ] I asked one or more LLMs to review the PR
- [ ] I asked one or more LLMs to check if this PR can be simplified [^1]
- [ ] If appropriate, I added tests for the changes in this PR
- [ ] If appropriate, I added performance benchmarks for the APIs added in this PR [^2]

[^1]: Sample prompt: "Can this be simplified? Can the code, comments, or logic introduced by these changes be simplfied, clarified or otherwise made more terse, concise and understandable, without affecting functionality?"
[^2]: `cargo bench -p nexus-webapi`
