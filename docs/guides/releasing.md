# Releasing Nexus

A release is a version number in the manifests and a `vX.Y.Z` tag on `main`. The tag publishes the Docker image and creates the GitHub release, whose notes are generated from the commit messages. The Rust crates are not published.

## Choosing the version

[versioning.md](versioning.md) says what a version number promises. Against that:

- **Major:** a `/v1` route, parameter or field is removed or changes meaning or type.
- **Minor:** something is added to `/v1`, or `/v0` is removed after its announced release.
- **Patch:** fixes only.

A change to the config file, the `nexusd` commands or the stored data does not decide the version, but its commit says what an operator has to do (below).

## Release notes come from the commits

Pull requests are squash-merged, so each one is a single commit on `main` with a [conventional commit](https://www.conventionalcommits.org) title: `feat(webapi): …`, `fix(watcher): …`. The release job lists those commits, grouped by type, from the previous tag to this one. A commit whose title is not conventional is left out.

When a change asks something of an operator, its squash commit carries a `BREAKING CHANGE:` footer that says what to do. The release notes show these footers first. That covers:

- a config field renamed, moved, removed or with a new default (the watcher ignores a field it does not know, so nothing else warns);
- a `nexusd` command or flag changed;
- a migration that needs downtime or an order of steps. A migration that only needs `nexusd db migration run` needs no footer: `nexusd db migration check` lists it.

```text
refactor(config): move the testnet fields to [stack.net] (#NNNN)

BREAKING CHANGE: `testnet` and `testnet_host` moved from `[watcher]` to
`[stack.net]`. A value left under `[watcher]` is ignored without an error.
```

## Steps

1. **Bump.** Branch from the current `main`. Set `version` under `[workspace.package]` in the root `Cargo.toml`; every crate takes it from there. Run `cargo check` so `Cargo.lock` follows. Open the pull request as `chore: vX.Y.Z` and merge it.
2. **Migrations.** Before tagging, the hosted deployment has run every migration the release registers: `nexusd db migration check` exits 0 there.
3. **Tag.** Tag the merge commit and push the tag:

   ```bash
   git tag vX.Y.Z <merge commit>
   git push <remote> vX.Y.Z
   ```

The tag starts `.github/workflows/docker.yml`. There is no approval step: pushing the tag publishes.

1. **Version check.** The tag, without its `v`, must be the version of every crate. When it is not, the workflow stops before publishing anything.
2. **Image.** It builds the image for `linux/amd64` and `linux/arm64` and pushes it under three tags: `synonymsoft/pubky-nexus:vX.Y.Z`, `synonymsoft/pubky-nexus:latest` and the full hash of the tagged commit. The tags come from the shared `build_and_push` action in `pubky/ci-workflows`, which `docker.yml` uses at its `main` branch. The commit-hash tag is that action's default. A change to the action can change the tags without a change in this repository.
3. **GitHub release.** Once the image is pushed, it creates the `vX.Y.Z` release with the notes generated from the commits.

To add text above the generated notes, such as an announcement, edit the release: `gh release edit vX.Y.Z --notes-file <file>`.

For 1.0.0, replace the generated notes with a short summary of the release. No 0.4.x version was tagged, so they list every commit since `v0.3.1`.

## Only the newest line gets a tag

`docker.yml` moves `latest` on every tag. A fix tagged on an older line, `v1.0.1` after `v1.1.0`, would move `latest` back to it. Fix the newest line and release that. If an older line ever needs a fix, change `docker.yml` first so its tag leaves `latest` alone.

The same goes for a prerelease tag such as `v2.0.0-rc.1`: the workflow matches it and moves `latest` too.

## After the tag

- The workflow run is green, and `docker pull synonymsoft/pubky-nexus:vX.Y.Z` works on both `linux/amd64` and `linux/arm64`.
- A deployment of the image reports the version at `/v0/info` and `/v1/info`, and in `info.version` of `/api-docs/v1/openapi.json`.
- The GitHub release exists, and its notes list the release's commits.

## A bad release

A published `vX.Y.Z` is never pushed again, as a git tag or as an image. Fix it on `main` and release the next patch version. Say in its fix commit, or at the top of its release notes, what was wrong with the bad one.

A tag the version check rejects published nothing. Delete it with `git push <remote> :refs/tags/vX.Y.Z` and push the tag that matches the manifests.

When the image is pushed but the release job fails, create the release from the same tag by hand: `gh release create vX.Y.Z --title vX.Y.Z --notes-file <file>`.
