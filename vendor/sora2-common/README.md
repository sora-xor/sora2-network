# sora2-common
## Release process
The repository adopts [GitHub Flow](https://docs.github.com/en/get-started/quickstart/github-flow) model with `develop` branch as default.
The `develop` branch is anticipated to be the latest stable version.
- Create branch `release/X.Y.Z` from `develop`, use [semver](https://semver.org/)
- `cargo update`
- Merge `release/X.Y.Z` into `develop`
- Create GitHub Release on `develop` branch, tag is `X.Y.Z`
