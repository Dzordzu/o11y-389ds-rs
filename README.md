# o11y-389ds-rs

![Exporter 389ds version](https://img.shields.io/github/v/tag/dzordzu/o11y-389ds-rs?filter=exporter*\&label=version)
![Nagios 389ds version](https://img.shields.io/github/v/tag/dzordzu/o11y-389ds-rs?filter=nagios*\&label=version)
![Haproxy 389ds version](https://img.shields.io/github/v/tag/dzordzu/o11y-389ds-rs?filter=haproxy*\&label=version)
![Config 389ds version](https://img.shields.io/github/v/tag/dzordzu/o11y-389ds-rs?filter=config*\&label=version)
![GitHub last commit (branch)](https://img.shields.io/github/last-commit/Dzordzu/o11y-389ds-rs/master)
![GitHub Release Date](https://img.shields.io/github/release-date/dzordzu/o11y-389ds-rs)

**Observability for 389ds (rust version)**

<!-- vim-markdown-toc GFM -->

* [What is 389ds?](#what-is-389ds)
* [Installation](#installation)
    * [From source](#from-source)
    * [Binaries and packages](#binaries-and-packages)
* [Observability](#observability)
    * [Supported features](#supported-features)
    * [Grafana dashboards](#grafana-dashboards)
    * [Exporter usage](#exporter-usage)
    * [Exporter result](#exporter-result)
    * [Nagios plugin usage](#nagios-plugin-usage)
    * [Configuration](#configuration)
        * [Notation](#notation)
        * [Definition](#definition)
* [Building and packaging](#building-and-packaging)
    * [Build dependencies](#build-dependencies)
* [Development](#development)
    * [Pre-commit hooks](#pre-commit-hooks)
    * [Commiting](#commiting)
* [Similar projects](#similar-projects)

<!-- vim-markdown-toc -->

## What is 389ds?

389ds is an LDAP server and the drop-in replacement for RedHat dirsrv.

## Installation

### From source

See [building](#building-and-packaging) section

### Binaries and packages

Each tag is also a release. The package is manually built and uploaded.
Currently, we only support RPM based package managers (dnf/yum). See
[releases](https://github.com/dzordzu/o11y-389ds-rs/releases).

## Observability

This repository contains the following projects

* `exporter-389ds-rs`: Prometheus exporter for the 389ds
* `nagios-389ds-rs`: Nagios plugin for the 389ds.
* `haproxy-389ds-rs`: HAProxy healthcheck plugin for the 389ds

### Supported features

* `cn=monitor` based checks and metrics (called `ldap-monitor` and `ldap_monitoring`)
* connection metrics with labeled information about connection DN and IP
  address
* replication based checks and metrics
* GID number metrics and checks - missing primary GIDs
* `dsctl` command based metrics and checks
* custom command metrics and checks
* integrity checks of custom commands

### Grafana dashboards

In addition to the binaries this repository also provides grafana dashboards.
You can see them inside the [grafana-389ds-rs](./grafana-389ds-rs) directory.

![Exporter dashboard](./grafana-389ds-rs/389ds-exporter.png)

### Exporter usage

```
Usage: exporter-389ds-rs [OPTIONS]

Options:
  -c, --config <CONFIG>
  -P, --page-size <PAGE_SIZE>
  -C, --skip-cert-verification
          Disable TLS cert verification
  -a, --expose-address <EXPOSE_ADDRESS>
  -p, --expose-port <EXPOSE_PORT>
  -b, --basedn <BASEDN>
  -D, --binddn <BINDDN>
  -w, --bindpass <BINDPASS>
  -H, --host <HOST>
  -I, --scrape-interval-seconds <SCRAPE_INTERVAL_SECONDS>
  -e, --enable-flags <ENABLE_FLAGS>
          [possible values: replication, ldap-monitor, gids-info, dsctl]
  -d, --disable-flags <DISABLE_FLAGS>
          [possible values: replication, ldap-monitor, gids-info, dsctl]
  -h, --help
          Print help (see more with '--help')
```

### Exporter result

See [exporter.result.txt](https://raw.githubusercontent.com/dzordzu/o11y-389ds-rs/master/exporter.result.txt)

### Nagios plugin usage

```
Perform nagios checks on the 389ds. All limits are using >= or <= comparsions, unless stated otherwise

Usage: check_389ds_rs [OPTIONS] <COMMAND>

Commands:
  check-int-metric        Check any scraped metric (integers). Fallback for missing options
  agreement-status        Check status of the replication
  agreement-skipped       Check skipped entries in the replication
  agreement-duration      Check duration of the replication
  missing-gids            Check if there are primary gids that are not present as posixGroup
  connections             Check number of active connections
  errors                  Check cumber of errors: Errors + SecurityErrors + BindSecurityErrors
  recent-restart          Check if daemon has been recently restarted
  diskspace               Check if disk space is low (declared by the daemon)
  cli-healthcheck         Check health using dsctl cli
  custom-query-time       Check custom query times
  custom-query-integrity  Check custom query integrity
  help                    Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>
  -C, --skip-cert-verification  Disable TLS cert verification
  -H, --host <HOST>
  -D, --binddn <BINDDN>
  -w, --bindpass <BINDPASS>
  -b, --basedn <BASEDN>
  -P, --page-size <PAGE_SIZE>
  -h, --help                   Print help
```

### Configuration

Both the exporter and the nagios plugin will rather work **without** any
configuration. To configure more checks/metrics and support non-standard
deployments, tools can be configured by TOML file or CLI options. Example file
can be found int the root of the repository. Every key below is ***optional***,
unless stated otherwise.

We provide a package that installs `default.toml` with proper permissions.

**TLDR;** See example [ldap-config.example.toml](https://raw.githubusercontent.com/dzordzu/o11y-389ds-rs/master/ldap-config.example.toml)

#### Notation

* Primitive types: `<string>`, `<int>`, `<bool>`
* Arrays/Vectors of type `SType`: `<[Stype]>`
* Maps with keys `KType` and values `VType`: `<map[KType, VType]>`
* Required field of type `RType`: `:<RType:required>`
* Variant `X` of the enum `E`: `<E::X`>
* Default value of the type `T`: `T::default`

#### Definition

```
ldap_uri = <string>                                   # default: ldap://localhost
default_base = <string>                               # default: (auto-detected)
verify_certs = <bool>                                 # default: true
page_size = <int>                                     # default: 999

bind = <BIND>                                         # default: None
scrapers = <SCRAPERS>                                 # default: SCRAPERS::default

haproxy = <HAPROXY>                                   # default: HAPROXY::default
exporter = <EXPORTER>                                 # default: EXPORTER::default
```

**\<BIND> type**

```
dn = <string:required>
pass = <string:required>
```

**\<SCRAPERS> type**

```
dsctl = <DSCTL>                                       # default: DSCTL::default
query = <[QUERY]>                                     # default: []
```

**\<DSCTL> type**

```
instance = <string>                                   # default: localhost
timeout_seconds = <int>                               # default: 10
```

**\<QUERY> type**

```
name = <string:required>
filter = <string:required>
max_entries = <int>                                   # default: (all possible entries)

attrs = <[string]>                                    # default: (all attributes)

# ---------------------------
# Overrides for main ldap config
verify_certs = <bool>                                 # default: None
uri = <string>                                        # default: None
page_size = <int>                                     # default: None
default_base = <string>                               # default: None
bind = <BIND>                                         # default: None
# ---------------------------
```

**\<HAPROXY> type**

```
expose_port = <int>                                   # default: 9966
expose_address = <string>                             # default: 0.0.0.0
query = <[HAPROXY_QUERY]>                             # default: []
scrape_flags = <map[<string>, HAPROXY_SCRAPE_FLAGS]>  # default: []
scrape_interval_seconds = <SCRAPE_INTERVALS>          # default: SCRAPE_INTERVALS::default>
```


**\<HAPROXY\_SCRAPE\_FLAGS> type**

```
replication_status = <bool>                           # default: true
ldap_monitoring = <bool>                              # default: true
```

**\<SCRAPE\_INTERVALS> type**

```
replication_status = <int>                            # default: 5
ldap_monitoring = <int>                               # default: 5
systemd_status = <int>                                # default: 5
```

**\<HAPROXY\_QUERY> type**

Enum. One of the following:

* `<HAPROXY_QUERY::COUNT_ENTRIES>`
* `<HAPROXY_QUERY::COUNT_ATTRS>`
* `<HAPROXY_QUERY::SUCCESS>`

**\<HAPROXY\_QUERY::COUNT\_ENTRIES> type**

```
name = <string:required>
action = "count-entries"
greater_than = <int>                                  # default: 0
less_than = <int>                                     # default: 0
scrape_interval_seconds = <int>                       # default: 30
```

**\<HAPROXY\_QUERY::COUNT\_ATTRS> type**

```
name = <string:required>
action = "count-attrs"
greater_than = <int>                                  # default: 0
less_than = <int>                                     # default: 0
scrape_interval_seconds = <int>                       # default: 5
```

**\<HAPROXY\_QUERY::SUCCESS> type**

```
name = <string:required>
action = "success"
scrape_interval_seconds = <int>                       # default: 5
```

**\<EXPORTER> type**

```
expose_port = <int>                                   # default: 9100
expose_address = <string>                             # default: 0.0.0.0
scrape_flags = <EXPORTER\_SCRAPE_FLAGS>               # default: EXPORTER_SCRAPE_FLAGS::default
query = <[EXPORTER_QUERY]>                            # default: []
scrape_interval_seconds = <int>                       # default: 5
```

**\<EXPORTER\_SCRAPE\_FLAGS> type**

```
replication_status = <bool>                           # default: true
ldap_monitoring = <bool>                              # default: true
gids_info = <bool>                                    # default: false
dsctl = <bool>                                        # default: false
```

**\<EXPORTER\_QUERY> type**

```
name = <string:required>
scrape_interval_seconds = <int>                       # default: 5
max_entries = <int>                                   # default: (all possible entries)
```

## Building and packaging

```bash
cargo xtask dist
```

### Build dependencies

* The binaries are made ONLY for linux based monitoring/389ds. They may work for
  other systems, but have not been tested.
* `rustc` and `cargo`
* The binaries are built against `musl` target

## Development

### Pre-commit hooks

To setup git hooks run

```bash
cargo xtask setup-repo
```

* `taplo` for the toml formatting
* `gitleaks` for passwords in repo detection

### Commiting

* Pre commit hooks check for different formatting issues and passwords in the
  repository.
* In order to fix formatting issues run `cargo xtask fmt`.

## Similar projects

* [389DS-exporter](https://github.com/ozgurcd/389DS-exporter) : Prometheus
  exporter for 389ds `cn=monitor` without connections and replication
  metrics. Data is gathered on each and every request.
* [`check_389ds_replication`](https://ypbind.de/maus/projects/check_389ds_replication/index.html#_check_389ds_replication):
  Nagios plugin for 389ds replication
* [`check_ldap_monitor_389ds`](https://github.com/ltb-project/nagios-plugins/blob/master/check_ldap_monitor_389ds.pl):
  nagios checks for common `cn=monitor`
* [Documentation based nagios checks](https://www.port389.org/docs/389ds/howto/howto-replicationmonitoring.html)
