# Releasing Nexus

A release is a version number in the manifests, a section in [CHANGELOG.md](../../CHANGELOG.md) and a `vX.Y.Z` tag on `main`. The tag publishes the Docker image. The Rust crates are not published.

## Choosing the version

The README says what a version number promises, under Versioning. Against that:

- **Major:** a `/v1` route, parameter or field is removed or changes meaning or type.
- **Minor:** something is added to `/v1`, or `/v0` is removed after its announced release.
- **Patch:** fixes only.

A change to the config file, the `nexusd` commands or the stored data does not decide the version, but it is listed under **Upgrade**.

## Steps

1. **Bump.** Branch from the current `main`. Set `version` under `[workspace.package]` in the root `Cargo.toml`; every crate takes it from there. Run `cargo check` so `Cargo.lock` follows.
2. **Changelog.** Add the release's section to `CHANGELOG.md` in the same commit, newest first, with:
   - **Upgrade:** what an operator does before or after starting the release: migrations to run, config fields renamed, removed or with a new default, commands changed. Write "Nothing" when there is nothing.
   - **API:** what changed in `/v1` and `/v0`, and an announced removal of `/v0`.
   - **Fixes** and anything else worth knowing.
3. **Pull request.** One pull request with the bump and the changelog. Merge it.
4. **Migrations.** Before tagging, the hosted deployment has run every migration the release registers: `nexusd db migration check` exits 0 there.
5. **Tag.** Tag the merge commit and push the tag:

   ```bash
   git tag vX.Y.Z <merge commit>
   git push <remote> vX.Y.Z
   ```

   The tag starts `.github/workflows/docker.yml`, which builds the image for `linux/amd64` and `linux/arm64` and pushes `synonymsoft/pubky-nexus:vX.Y.Z` and `synonymsoft/pubky-nexus:latest`. There is no approval step: pushing the tag publishes.
6. **GitHub release.** Create it from the tag by hand, with the release's changelog section as its text:

   ```bash
   gh release create vX.Y.Z --title vX.Y.Z --notes-file <section.md>
   ```

## Only the newest line gets a tag

`docker.yml` moves `latest` on every tag. A fix tagged on an older line, `v1.0.1` after `v1.1.0`, would move `latest` back to it. Fix the newest line and release that. If an older line ever needs a fix, change `docker.yml` first so its tag leaves `latest` alone.

The same goes for a prerelease tag such as `v2.0.0-rc.1`: the workflow matches it and moves `latest` too.

## After the tag

- The workflow run is green, and both images pull: `docker pull synonymsoft/pubky-nexus:vX.Y.Z`.
- A deployment of the image reports the version at `/v0/info` and `/v1/info`, and in `info.version` of `/api-docs/v1/openapi.json`.
- The GitHub release exists and its text is the changelog section.

## A bad release

A published `vX.Y.Z` is never pushed again, as a git tag or as an image. Fix it on `main` and release the next patch version. Say in its changelog section what was wrong with the bad one.
