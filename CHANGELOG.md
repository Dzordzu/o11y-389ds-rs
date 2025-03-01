# CHANGELOG

## grafana (2025-03-01)

* Added more comments to the existing dashboards

## exporter-389ds-rs-0.1.5 (2025-03-01)

* Fixed unit removal
* Fixed clean connecion close
* Added binary sha256 checksums to the dist
* Added health info about scrapes enabled by particular flags
* Added ldap\_uri to the exported labels
* Added exporter info

## nagios-389ds-rs-0.1.4 (2025-03-01)

* Fixed clean connecion close
* Added binary sha256 checksums to the dist

## exporter-389ds-rs-0.1.4 (2025-02-28)

* Fixed error level parsing for cli checks
* Fixed cli parsed false-positives (added in-memory storage for them)

## nagios-389ds-rs-0.1.3 (2025-02-28)

* Fixed error level parsing for cli checks

## exporter-389ds-rs-0.1.3 (2025-02-27)

* Now config is preserved during update

## exporter-389ds-rs-0.1.2 (2025-02-27)

* More information on the failing cli checks
* Now dsctl uses sudo
* Added sudoers file for dsctl command

## nagios-389ds-rs-0.1.2 (2025-02-27)

* More information on the failing cli checks
* Now dsctl uses sudo
