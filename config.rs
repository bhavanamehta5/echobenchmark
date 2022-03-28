// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use anyhow::{format_err, Error, Result};
use catnip::protocols::{ethernet2::MacAddress, ip::Port, ipv4, ipv4::Endpoint};
use demikernel::config::Config;
use std::collections::HashMap;
use std::{convert::TryFrom, ffi::CString, net::Ipv4Addr, str::FromStr};

//==============================================================================
// ConfigFile
//==============================================================================

#[derive(Debug)]
pub struct ConfigFile {
    pub config: Config,
}

impl ConfigFile {
    pub fn initialize(config_path: &str) -> Result<Self, Error> {
        let config = Config::new(config_path.to_string());

        Ok(Self { config })
    }

    pub fn local_link_addr(&self) -> MacAddress {
        self.config.local_link_addr
    }

    pub fn interface_name(&self) -> String {
        self.config.local_interface_name.to_string()
    }

    pub fn local_ipv4_addr(&self) -> Ipv4Addr {
        self.config.local_ipv4_addr
    }

    pub fn arp_table(&self) -> HashMap<Ipv4Addr, MacAddress> {
        self.config.arp_table()
    }

    pub fn disable_arp(&self) -> bool {
        self.config.disable_arp
    }

    pub fn eal_init_args(&self) -> Vec<CString> {
        self.config.eal_init_args()
    }

    pub fn server_addr(&self) -> ipv4::Endpoint {
        match self.addr("server", "bind") {
            Ok(addr) => addr,
            Err(_) => self
                .addr("client", "connect_to")
                .expect("bad server address"),
        }
    }

    pub fn client_addr(&self) -> ipv4::Endpoint {
        match self.addr("server", "client") {
            Ok(addr) => addr,
            Err(_) => self.addr("client", "client").expect("bad client address"),
        }
    }

    pub fn addr(&self, k1: &str, k2: &str) -> Result<Endpoint, Error> {
        let addr = &self.config.config_obj[k1][k2];
        let host_s = addr["host"].as_str().ok_or(format_err!("Missing host"))?;
        let host = Ipv4Addr::from_str(host_s)?;
        let port_i = addr["port"].as_i64().ok_or(format_err!("Missing port"))?;
        let port = Port::try_from(port_i as u16)?;
        Ok(Endpoint::new(host, port))
    }
}
