use core::fmt;
use std::borrow::Cow;
use std::path::Path;

use url::Url;

use crate::FQDN;

#[derive(Clone, Copy)]
pub enum Config<'a> {
    NameServer { origin: &'a FQDN },
    Resolver { use_dnssec: bool, netmask: &'a str },
}

impl Config<'_> {
    pub fn role(&self) -> Role {
        match self {
            Config::NameServer { .. } => Role::NameServer,
            Config::Resolver { .. } => Role::Resolver,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Role {
    NameServer,
    Resolver,
}

impl Role {
    #[must_use]
    pub fn is_resolver(&self) -> bool {
        matches!(self, Self::Resolver)
    }
}

#[derive(Clone)]
pub enum Implementation {
    Bind,
    Hickory(Repository<'static>),
    Unbound,
}

impl Implementation {
    #[must_use]
    pub fn is_bind(&self) -> bool {
        matches!(self, Self::Bind)
    }

    pub(crate) fn format_config(&self, config: Config) -> String {
        match config {
            Config::Resolver {
                use_dnssec,
                netmask,
            } => match self {
                Self::Bind => {
                    minijinja::render!(
                        include_str!("templates/named.resolver.conf.jinja"),
                        use_dnssec => use_dnssec,
                        netmask => netmask,
                    )
                }

                Self::Hickory(_) => {
                    minijinja::render!(
                        include_str!("templates/hickory.resolver.toml.jinja"),
                        use_dnssec => use_dnssec,
                    )
                }

                Self::Unbound => {
                    minijinja::render!(
                        include_str!("templates/unbound.conf.jinja"),
                        use_dnssec => use_dnssec,
                        netmask => netmask,
                    )
                }
            },

            Config::NameServer { origin } => match self {
                Self::Bind => {
                    minijinja::render!(
                        include_str!("templates/named.name-server.conf.jinja"),
                        fqdn => origin.as_str()
                    )
                }

                Self::Unbound => {
                    minijinja::render!(
                        include_str!("templates/nsd.conf.jinja"),
                        fqdn => origin.as_str()
                    )
                }

                Self::Hickory(_) => unimplemented!(),
            },
        }
    }

    pub(crate) fn conf_file_path(&self, role: Role) -> &'static str {
        match self {
            Self::Bind => "/etc/bind/named.conf",

            Self::Hickory(_) => "/etc/named.toml",

            Self::Unbound => match role {
                Role::NameServer => "/etc/nsd/nsd.conf",
                Role::Resolver => "/etc/unbound/unbound.conf",
            },
        }
    }

    pub(crate) fn cmd_args(&self, role: Role) -> &[&'static str] {
        match self {
            Implementation::Bind => &["named", "-g", "-d5"],

            Implementation::Hickory(_) => {
                assert!(
                    role.is_resolver(),
                    "hickory acting in `NameServer` role is currently not supported"
                );

                &["hickory-dns", "-d"]
            }

            Implementation::Unbound => match role {
                Role::NameServer => &["nsd", "-d"],

                Role::Resolver => &["unbound", "-d"],
            },
        }
    }

    pub(crate) fn pidfile(&self, role: Role) -> &'static str {
        match self {
            Implementation::Bind => "/tmp/named.pid",

            Implementation::Hickory(_) => unimplemented!(),

            Implementation::Unbound => match role {
                Role::NameServer => "/tmp/nsd.pid",
                Role::Resolver => "/tmp/unbound.pid",
            },
        }
    }
}

impl fmt::Display for Implementation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Implementation::Bind => "bind",
            Implementation::Hickory(_) => "hickory",
            Implementation::Unbound => "unbound",
        };

        f.write_str(s)
    }
}

#[derive(Clone)]
pub struct Repository<'a> {
    inner: Cow<'a, str>,
}

impl Repository<'_> {
    pub(crate) fn as_str(&self) -> &str {
        &self.inner
    }
}

/// checks that `input` looks like a valid repository which can be either local or remote
///
/// # Panics
///
/// this function panics if `input` is not a local `Path` that exists or a well-formed URL
#[allow(non_snake_case)]
pub fn Repository(input: impl Into<Cow<'static, str>>) -> Repository<'static> {
    let input = input.into();
    assert!(
        Path::new(&*input).exists() || Url::parse(&input).is_ok(),
        "{input} is not a valid repository"
    );
    Repository { inner: input }
}

impl Default for Implementation {
    fn default() -> Self {
        Self::Unbound
    }
}
