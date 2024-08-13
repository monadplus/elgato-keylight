# Development

## Pacman packaging

First, install `devtools`. And, then run `$ pkgctl build` on the root. This will try building your package in a [clean chroot](https://wiki.archlinux.org/title/DeveloperWiki:Building_in_a_clean_chroot).

- You can verify your `PKGBUILD` with:
  - `$ namcap PKGBUILD`
  - `shellcheck --shell=bash --exclude=SC2034,SC2154 PKGBUILD`
- You can verify your package with `$ namcap elgato-keylight-0.4.0-1-x86_64.pkg.tar.zst`
