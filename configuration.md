# TOML Configuration file

Exporter, haproxy agent and the nagios plugin will most likely without any
configuration. To configure more checks/metrics and support non-standard
deployments, tools can be configured by TOML file or CLI options. Example file
can be found int the root of the repository. Every key below is optional,
unless stated otherwise.

## Table of contents
<!-- vim-markdown-toc GFM -->

* [Notation](#notation)
* [`config.toml` format](#configtoml-format)
* [Types](#types)

<!-- vim-markdown-toc -->

## Notation

* Primitive types: `<string>`, `<int>`, `<bool>`
* Arrays/Vectors of type `SType`: `<[Stype]>`
* Required field of type `RType`: `:<RType:required>`
* Variant `X` of the enum `E`: `<E::X`>
* Default value of the type `T`: `T::default`

## `config.toml` format

```toml
ldap_uri = <string>                             # default: ldap://localhost
default_base = <string>                         # default: (auto-detected)

verify_certs = <bool>                           # default: true
page_size = <int>                               # default: 999
scrape_interval_seconds = <int>                 # default: 5
bind = <BIND>                                   # default: None
dsctl = <DSCTL>                                 # default: DSCTL::default
exporter = <EXPORTER>                           # default: EXPORTER::default
haproxy = <HAPROXY>                             # default: HAPROXY::default
query = <[QUERY]>                               # default: []
```

## Types
