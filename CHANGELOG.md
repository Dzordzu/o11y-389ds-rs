# CHANGELOG

## exporter-389ds-rs-0.2.3 (2025-10-07)
* Dependency updates (major)

## nagios-389ds-rs-0.2.3 (2025-10-07)
* Dependency updates (major)

## haproxy-389ds-rs-0.2.3 (2025-10-07)
* Dependency updates (major)

## exporter-389ds-rs-0.2.2 (2025-08-12)
* Dependency updates

## haproxy-389ds-rs-0.2.2 (2025-08-12)
* Dependency updates
* Added missing tcp server
* Updated docs

## nagios-389ds-rs-0.2.2 (2025-08-12)
* Dependency updates

## exporter-389ds-rs-0.2.1 
* Removed single println from the code

## nagios-389ds-rs-0.2.1
* Removed single println from the code

## haproxy-389ds-rs-0.2.1
* Removed single println from the code

## config-389ds-rs-0.2.0 (2025-06-30)
* Migrated default config to config-389ds-rs

## exporter-389ds-rs-0.2.0 (2025-06-30)
* New config
* Migrated default config to config-389ds-rs
* Now queries are executed in a separate loops

## nagios-389ds-rs-0.2.0 (2025-06-30)
* New config

## haproxy-389ds-rs-0.2.0 (2025-06-30)
* Added haproxy-389ds-rs

## grafana-389ds-rs-0.2.0 (2025-06-30)
* Nothing new actually... TBD

## nagios-389ds-rs-0.1.6 (2025-04-30)

* Fixed issue with dsctl spawning. Now only one dsctl is spawned at one moment.
  Old ones are killed
* Now logs are excluded from healtchecks

## exporter-389ds-rs-0.1.10 (2025-04-30)

* Fixed issue with blocking behavior of dsctl spawning introduced in 0.1.9
* Now logs are excluded from healtchecks

## exporter-389ds-rs-0.1.9 (2025-04-30)

* Fixed issue with dsctl spawning. Now only one dsctl is spawned at one moment.
  Old ones are killed

## exporter-389ds-rs-0.1.8 (2025-03-11)

* Fixed issue with version metric (old versions were kept)

## exporter-389ds-rs-0.1.7 (2025-03-05)

* Updated cargo dependencies
* Added more checksums

## nagios-389ds-rs-0.1.5 (2025-03-05)

* Updated cargo dependencies
* Added more checksums

## grafana-389ds-rs-0.1.0 (2025-03-05)

* Now releases include grafana dashboards
* Added new dashboard
* Fixed datasources in the original one

## exporter-389ds-rs-0.1.6 (2025-03-04)

* Fixed issue with exporter timer (always counting 5s, regardless of the
  interval)

## grafana (2025-03-03)

* Added ldap\_uri filter

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
