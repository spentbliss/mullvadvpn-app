# Changelog
All changes to the software that can be noticed from the users' perspective should have an entry in
this file. Except very minor things that will not affect functionality, such as log message changes
and minor GUI adjustments.

### Format

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/).

Entries should have the imperative form, just like commit messages. Start each entry with words like
add, fix, increase, force etc.. Not added, fixed, increased, forced etc.

Line wrap the file at 100 chars.                                              That is over here -> |

### Categories each change fall into

* **Added**: for new features.
* **Changed**: for changes in existing functionality.
* **Deprecated**: for soon-to-be removed features.
* **Removed**: for now removed features.
* **Fixed**: for any bug fixes.
* **Security**: in case of vulnerabilities.

## Unreleased
### Added
- Make feature indicators clickable shortcuts to their corresponding settings.
- Let users cancel sending a problem report.
- Add possibility to manage devices from account view.
- Add support for Dynamic Type to allow fonts to scale according to user's system settings.
- Add notification that shows when the user is connected to WireGuard with a port that is not 
  supported.

### Changed
- Replace Classic McEliece with HQC as one of the post-quantum safe key exchange
  mechanisms used for the quantum-resistant tunnels. The main benefits here are that HQC
  uses a lot less CPU to compute the keypair, and the public key sent to the server
  is drastically smaller.

## [2025.5 - 2025-06-17]
### Changed
- Make the app feel more responsive when reconnecting.

### Fixed
- Fix app thinking the device is offline when another VPN is already active.

## [2025.4 - 2025-05-20]
### Added
- Make account number copyable on welcome screen.
- Add animations for connection view.

### Changed
- Improve the filter view to display the number of available servers based on selected criteria.
- Improve location view to filter out servers not compatible with custom obfuscation port.

## [2025.3 - 2025-03-06]
### Fixed
- Fix DAITA for multihop.

## [2025.2 - 2025-02-08]
### Added
- Add different themes for app icons

### Fixed
- Broken DAITA settings view on iOS 15.

### Changed
- Move changelog to settings and add an in-app notification banner for app update.

### Removed
- Remove Google's resolvers from encrypted DNS proxy.

## [2025.1 - 2025-01-14]
### Added
- Update to DAITA v2 - now machines are provided by relays dynamically instead
  of using bundled ones.

## [2024.11 - 2024-12-12]
### Added
- Add WireGuard over Shadowsocks obfuscation. It can be enabled in "VPN settings". This will
  also be used automatically when connecting fails with other methods.
- Add new settings views for DAITA and multihop.

### Fixed
- When loading logs, a spinner will be shown to indicate the app is busy.

## [2024.10 - 2024-11-20]
### Fixed
- Removed deadlock when losing connectivity without entering offline state.
- Improved log reporting.

## [2024.9 - 2024-11-07]
### Added
- DAITA everywhere, using multihop.

### Changed
- Replace the draft key encapsulation mechanism Kyber (round 3) with the standardized
  ML-KEM (FIPS 203) dito in the handshake for Quantum-resistant tunnels.

### Fixed
- Fix app going into blocked state on first-time installs.

## [2024.8 - 2024-10-14]
### Added
- Add a new access method that uses the encrypted DNS proxy to reach our API.

### Fixed
- Fix IPv6 parsing in API access

## [2024.7 - 2024-09-16]
### Added
- Add DAITA (Defence against AI-guided Traffic Analysis) setting.

## [2024.6 - 2024.09-02]
### Fixed
- Fixed multihop in networks that use DNS64 and NAT64.

## [2024.5 - 2024-08-19]
### Added
- Add multihop, a feature that routes traffic through two
  of our relays before it reaches the internet.

## [2024.4 - 2024-06-25]
### Added
- Add Post-Quantum secure tunnels.

## [2024.3 - 2024-05-13]
### Added
- Add ability to create custom lists.

## [2024.2 - 2024-02-26]
### Added
- Add IP Overrides.

## [2024.1 - 2024-01-06]
### Added
- Add custom API access methods.

## [2023.8 - 2023-12-08]
### Added
- Add UDP-over-TCP WireGuard obfuscation.

## [2023.7 - 2023-11-23]
### Added
- Add filtering on ownership and provider to location selection view.
- Add blocked state.
- Show exit IP when connected to a relay.


## [2023.6 - 2023-10-12]
### Removed
- Remove voucher redemption from the app.


## [2023.5 - 2023-09-22]
### Added
- A new option to block Social media.

### Fixed
- Fixed crash when deleting an account whilst connected.


## [2023.4 - 2023-09-12]
### Added
- Allow redeeming vouchers in account view.
- Allow deleting account in account view.
- Add new account flow.

### Fixed
- Invalidate API IP address cache to fix connectivity issues for some of devices updating from
  2023.2 or earlier.


## [2023.3 - 2023-07-15]
### Added
- Add search functionality to location selection view.
- Wipe all settings on app reinstall.
- Add a dedicated account button on the main view and remove it from settings.
- Rotate public key from within packet tunnel when it detects that the key stored on backend does
  not match the one stored on device.
- Add WireGuard port selection to settings.
- Add redeeming voucher code on account view.

## [2023.2 - 2023-04-03]
### Changed
- Changed key rotation interval from 4 to 14 days.
- Delay tunnel reconnection after a WireGuard private key rotates. Accounts for latency in key
  propagation to relays.
- Increase API request timeouts to improve usability in bad network conditions.


## [2023.1] - 2023-03-21
### Added
- Add option to block gambling and adult content.
- Add last used account field to login view.
- Display device name under account view.
- Add revoked device view displayed when the app detects that device is no longer registered on
  backend.
- Add ability to manage registered devices if too many devices detected during log-in.
- Add continuous monitoring of tunnel connection. Verify ping replies to detect whether traffic is
  really flowing.
- Check if device is revoked or account has expired when the tunnel fails to connect on each second
  failed attempt.

### Changed
- When logged into an account with no time left, a new view is shown instead of account settings,
with the option to buy more time.
- Use exponential backoff with jitter for delay interval when retrying REST API requests.
- REST API requests will bypass VPN when tunnel is not functional.

### Fixed
- Improve random port distribution. Should be less biased towards port 53.
- Fix invalid map camera position during the app launch and keep it up to date when multitasking.
- Fix animation glitch when expanding partially visible cell in location picker.
- Periodically refresh account expiry in-app notification.

## Removed
- Remove iOS 12 support.


## [2022.2] - 2022-04-28
### Added
- Add tunnel monitor when establishing tunnel connection. Picks next relay every 15 seconds until
  any inbound traffic received. This should also keep the tunnel in connecting or reconnecting state
  until the tunnel monitor determined that connection is functional.
- Add "FAQ & Guides" link in Settings.

### Changed
- Delete leftover settings in Keychain during login. WireGuard keys will be removed from
  server too if old settings can be read. This is usually the case when uninstalling the app and
  then reinstalling it without logging out first.
- Validate account token before charging user (in-app purchases). Safeguards from trying to add
  credits on accounts that no longer exist on our backend. Usually the case with newly created
  accounts that went stale.


## [2022.1] - 2022-02-15
### Added
- Show privacy overlay when entering app switcher.
- Add option to block malware.

### Fixed
- Fix crash occurring after completing in-app purchase.
- Fix error when changing relays while in airplane mode.
- Prevent key rotation from clogging the server key list by storing the next key and reusing it
  until receiving the successful response from Mullvad API. Add up to three retry attempts.

### Changed
- Increase hit area of settings (cog) button.
- Update launch screen.
- Never use DNS to talk to Mullvad API. Instead use the list of IP addresses bundled with the app
  and update it periodically.


## [2021.4] - 2021-11-30
### Added
- Add ability to specify custom DNS servers.

### Changed
- Attach log backup from previous application run to problem report.
- Use background tasks to periodically update relays and rotate the private key on iOS 13 or newer.
  Background fetch is used as fallback on iOS 12.
- Request background execution time from the system when performing critical tasks.

### Fixed
- Drop leading replacement characters (`\u{FFFD}`) when decoding UTF-8 from a part of log file.

### Security
- Move REST API networking from the packet tunnel process to the main process to prevent leaking
  traffic outside of the tunnel.


## [2021.3] - 2021-08-10
### Added
- Show a reminder to add more credits 3 days before account expiry via system notification and
  in-app message.
- Add submit button next to account input field on login screen.

### Fixed
- Update WireGuardKit to the latest. Fixes iOS 15 support.
- Improve accessibility support.


## [2021.2] - 2021-06-03
### Added
- Enable option to "Select all" when viewing app logs.
- Split view interface for iPad.
- Add interactive map.
- Reduce network traffic consumption by leveraging HTTP caching via ETag HTTP header to avoid
  re-downloading the relay list if it hasn't changed.
- Pin root SSL certificates.
- Add option to use Mullvad's ad-blocking DNS servers.

### Fixed
- Fix bug which caused the tunnel manager to become unresponsive in the rare event of failure to
  disable on-demand when stopping the tunnel from within the app.
- Fix bug that caused the app to skip tunnel settings migration from older versions of the app.
- Localize some of well known StoreKit errors so that they look less cryptic when presented to user.
- Improve tunnel settings verification to address issues with broken tunnel and missing Keychain
  entries to tunnel settings in cases such as when setting up a new device from backup.


## [2021.1] - 2021-03-16
### Added
- Add ability to report a problem inside the app. Sends logs to support.

### Changed
- Migrate to WireGuardKit framework.

### Fixed
- Fix crash when pasting empty string into account input field.
- Fix invalid initial text color of "unsecured connection" label on iOS 12.


## [2020.5] - 2020-11-04
### Fixed
- Fix regression where "Internal error" was displayed instead of server error (i.e too many
  WireGuard keys)


## [2020.4] - 2020-09-10
### Added
- Save application logs to file.
- Add button to reconnect the tunnel.
- Add support for iOS 12.
- Ship the initial relay list with the app, and do once an hour periodic refresh in background.
- Refresh account expiry when visiting settings.

### Fixed
- Fix the issue when starting the tunnel could take longer than expected due to the app refreshing
  the relay list before connecting.
- Fix the issue when regenerating the WireGuard key and dismissing the settings at the same
  time could lead to the revoked key still being used by the tunnel, leaving the tunnel unusable.

### Changed
- Remove the WireGuard key from the account inside the VPN tunnel during the log out, if VPN is
  active at that time. Before it would always remove it outside the tunnel.
- Turn off WireGuard backend when there are no active network interfaces available. Saves battery.
- Switch from JSON-RPC to REST communication protocol when talking to Mullvad API servers.


## [2020.3] - 2020-06-12
### Added
- Add automatic key rotation every 4 days.

### Fixed
- Fix relay selection for country wide constraints by respecting the `include_in_country`
  parameter.
- Fix defect when manually regenerating the private key from Settings would automatically connect
  the tunnel.
- Properly format date intervals close to 1 day or less than 1 minute. Enforce intervals between 1
  and 90 days to always be displayed in days quantity.
- Fix a number of errors in DNS64 resolution and IPv6 support.
- Update the tunnel state when the app returns from suspended state.
- Disable `URLSession` cache. Fixes audit finding [`MUL-02-001`]

[`MUL-02-001`]: ../audits/2020-06-12-cure53.md#miscellaneous-issues


## [2020.2] - 2020-04-16
### Fixed
- Fix "invalid account" error that was mistakenly reported as "network error" during log in.
- Fix parsing of pre-formatted account numbers when pasting from pasteboard on login screen.

### Added
- Format account number in groups of 4 digits separated by whitespace on login screen.
- Enable on-demand VPN with a single rule to always connect the tunnel when on Wi-Fi or cellular.
  Automatically disable on-demand VPN when manually disconnecting the tunnel from GUI to prevent the
  tunnel from coming back up.


## [2020.1] - 2020-04-08
Initial release. Supports...
* Establishing WireGuard tunnels
* Selecting and changing location and servers
* See account expiry
* Purchase more VPN time via in-app purchases
* See the current WireGuard key in use and how long it has been used
* Generate a new WireGuard key to replace the old
