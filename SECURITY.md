# Security

## Reporting a vulnerability

Please report it privately, not in a public issue: open the repository's **Security** tab and
choose **Report a vulnerability**
(<https://github.com/MaddisonM79/ryna_agent/security/advisories/new>). Only Maddison sees it.

Say what you found, in which part (the desktop app, the `ryna` CLI, the addon or a mod, or the
release and update pipeline), the version, and how to reproduce it. Please don't include anyone's
account names, character names or saved files.

## Supported versions

Only the latest release gets fixes. The app updates itself, so a fix ships as a new version.

## Checking a download

Every release is built by this repository's `release` workflow from a signed tag on `main`, and
nothing on a release is ever changed after it's published. Each release has a `SHA256SUMS`
file covering everything on it; the macOS app is signed and notarized. See
[agent/README.md](agent/README.md) for installing and for what each file is.
