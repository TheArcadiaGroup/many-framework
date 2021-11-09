use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::{Status, StatusBuilder};
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::OmniRequestHandler;
use crate::{Identity, OmniError};
use async_trait::async_trait;
use minicose::{CoseKey, Ed25519CoseKeyBuilder};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::collections::BTreeSet;

pub mod function;
pub mod module;

use crate::server::module::base::BaseServerModule;

#[derive(Debug, Clone)]
pub struct OmniModuleList {}

#[derive(Debug, Default)]
pub struct OmniServer {
    modules: Vec<Box<dyn OmniModule>>,
    method_cache: BTreeSet<&'static str>,
    identity: Identity,
    public_key: CoseKey,
}

impl OmniServer {
    pub fn new(identity: Identity, public_key: &Ed25519KeyPair) -> Self {
        debug_assert!(identity.is_addressable());

        let x = public_key.public_key().as_ref().to_vec();
        let public_key: CoseKey = Ed25519CoseKeyBuilder::default()
            .x(x)
            .build()
            .unwrap()
            .into();

        Self {
            identity,
            public_key,
            ..Default::default()
        }
        .with_module(BaseServerModule)
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: OmniModule + 'static,
    {
        let OmniModuleInfo { attributes, .. } = module.info();
        for a in attributes {
            let id = a.id;

            if let Some(m) = self
                .modules
                .iter()
                .find(|m| m.info().attributes.iter().any(|a| a.id == id))
            {
                panic!(
                    "Module {} already implements attribute {}.",
                    m.info().name,
                    id
                );
            }
        }

        for a in attributes {
            for e in a.endpoints.unwrap_or(&[]) {
                if self.method_cache.contains(e) {
                    unreachable!(
                        "Method '{}' already implemented, but there was no attribute conflict.",
                        e
                    );
                }
            }
        }

        // Update the cache.
        for a in attributes {
            for e in a.endpoints.unwrap_or(&[]) {
                self.method_cache.insert(e);
            }
        }
        self.modules.push(Box::new(module));
        self
    }

    pub fn status(&self) -> Status {
        StatusBuilder::default()
            .version(1)
            .public_key(self.public_key.clone())
            .identity(self.identity.clone())
            .internal_version(vec![])
            .attributes(vec![])
            .build()
            .unwrap()
    }

    pub fn endpoints(&self) -> Vec<&'static str> {
        self.method_cache.iter().map(|x| *x).collect()
    }
}

#[async_trait]
impl OmniRequestHandler for OmniServer {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = message.to;
        let method = message.method.as_str();

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || &self.identity == &to {
            // Verify the endpoint.
            if self.method_cache.contains(method) {
                Ok(())
            } else {
                Err(OmniError::invalid_method_name(method.to_string()))
            }
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.to_string(),
            ))
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = &message.method.as_str();

        if let Some(payload) = match message.method.as_str() {
            "status" => Some(
                self.status()
                    .to_bytes()
                    .map_err(|e| OmniError::serialization_error(e))?,
            ),
            "heartbeat" => Some(Vec::new()),
            "echo" => Some(message.data.clone()),
            "endpoints" => Some(
                minicbor::to_vec(self.endpoints())
                    .map_err(|e| OmniError::serialization_error(e.to_string()))?,
            ),
            _ => None,
        } {
            return Ok(ResponseMessage::from_request(
                &message,
                &self.identity,
                Ok(payload),
            ));
        }

        for m in &self.modules {
            let attrs = &m.info().attributes;
            if attrs
                .iter()
                .any(|a| a.endpoints.unwrap_or(&[]).contains(method))
            {
                return m.execute(message).await.map(|mut r| {
                    r.from = self.identity;
                    r
                });
            }
        }

        Err(OmniError::invalid_method_name(method.to_string()))
    }
}
