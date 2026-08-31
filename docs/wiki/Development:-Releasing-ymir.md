This is a checklist of things to release a new ymir version.

We'll use `1.0.0` as the example new version.
When making a patch release, append the patch number like `1.0.1`.

## Prepare the release notes

Plan for a few days of work, this usually takes a while.

During this process, also check:

- that all additions are marked with "1.0.0" (or the new version) on the wiki,
- if anything needs updating in `README.md`.

## Bump version

We use `major.minor.patch` semantic versioning. Ymir 1.0.0 is the first stable
release.

You can use the command from [cargo-edit](https://github.com/killercup/cargo-edit):

```
cargo set-version 1.0.0
```

Then, manually update version in:

- `[package.metadata.generate-rpm]` in Cargo.toml
- Dependency example in `ymir-ipc/README.md`
- Dependency example in `ymir-ipc/src/lib.rs`

Do a full text search for the old version to make sure there are no other places.

## Replace all "Since: next release" mentions

Do a full text search for `next release`, replace everything with the new version number.

## Build, test, push, and have the CI run

Run all tests:

```
RUN_SLOW_TESTS=1 cargo test --release --all
```

- Run `cargo package -p ymir-ipc` and make sure it succeeds.
- Make sure the CI passes.

## Create and push the release git tag

The tag starts with a `v`:

```
git tag -am "v1.0.0 release" v1.0.0
git push origin v1.0.0
```

Use an annotated tag: it plays better with various tooling than a lightweight tag.

## Publish the release

- Draft the release on the Forgejo instance (or your preferred mirror) with the release notes.
- If the release is built from a vendored dependency archive (see below), attach it to the release.

## Vendored dependencies

If you want to make an offline-buildable release, produce a vendored dependency archive:

```
cargo vendor
tar cJf ymir-vendored-deps.tar.xz vendor/
```

Build and test ymir against the archive before attaching it to the release.
Distro packagers can then use it to build the release completely offline.

## Update the RPM spec

`ymir.spec.rpkg` lives in the repository root.

- Update version global to `1.0.0`.
- Update commit global to the commit hash corresponding to the release commit.
You can use `git rev-parse HEAD`.
- Reset the `Release:` number to 1 if it was higher.

To run a test build, you can download the vendored dependency archive from the last step.
Comment/uncomment `Source:` and `%autosetup` lines accordingly.

During the build, the list of licenses is printed; update it in the spec accordingly.

If you had to update `ymir.spec.rpkg` and therefore make another commit to the ymir repo, make sure to update the commit hash in the spec global again.

## Publish the ymir-ipc crate

```
cargo publish -p ymir-ipc
```

## Announce the release

Chat rooms, social media, etc.

## Update wayland.app protocol data

- Install [wlprobe](https://github.com/PolyMeilex/wlprobe).
- Clone https://github.com/vially/wayland-explorer.
- Generate data:

    ```
    wlprobe > ./src/data/compositors/ymir.json
    ```

- Manually add `"version": "1.0.0"`, then clean up the diff from unrelated changes, for example:
    - The number of `wl_output`s will change depending on how many monitors you have connected.
    - The number of `wp_drm_lease_device_v1` will change depending on your number of GPUs.
    - `org_kde_kwin_server_decoration_manager` and `zxdg_decoration_manager_v1` will only appear with `prefer-no-csd`.
- Create a pull request.