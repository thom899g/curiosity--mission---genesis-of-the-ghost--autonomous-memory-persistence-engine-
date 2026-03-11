//! ego-core - The kernel of the Ghost autonomous memory persistence engine
//!
//! This module implements the core StateObject protocol and immutable log management
//! following the architectural principles of immutability and content-addressability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Core error types for the Ghost engine
#[derive(Error, Debug)]
pub enum GhostError {
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid state object: {0}")]
    InvalidState(String),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    #[error("Plugin error: {0}")]
    Plugin(String),
}

/// Result type alias for Ghost operations
pub type GhostResult<T> = Result<T, GhostError>;

/// Universal state envelope following content-addressable protocol
/// CID = Content Identifier (hash of encrypted data + metadata)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StateObject {
    /// Content Identifier - SHA256 hash of (encrypted_data + metadata_json)
    pub id: String,
    
    /// ISO 8601 timestamp in UTC
    pub timestamp: DateTime<Utc>,
    
    /// Source identifier (e.g., "harvester:fs", "harvester:process")
    pub source: String,
    
    /// Encrypted payload (compression optional, handled by plugins)
    pub encrypted_data: Vec<u8>,
    
    /// Metadata JSON (MIME type, annotations, lineage, encryption method)
    pub metadata: HashMap<String, serde_json::Value>,
    
    /// Optional cryptographic signature for verification
    pub signature: Option<Vec<u8>>,
}

impl StateObject {
    /// Create a new StateObject with automatic CID generation
    pub fn new(
        source: String,
        encrypted_data: Vec<u8>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> GhostResult<Self> {
        let timestamp = Utc::now();
        
        // Generate CID by hashing encrypted data + canonical metadata
        let cid = Self::generate_cid(&encrypted_data, &metadata)?;
        
        Ok(Self {
            id: cid,
            timestamp,
            source,
            encrypted_data,
            metadata,
            signature: None,
        })
    }
    
    /// Generate Content Identifier from data and metadata
    fn generate_cid(
        encrypted_data: &[u8],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> GhostResult<String> {
        let mut hasher = Sha256::new();
        
        // Hash the encrypted data
        hasher.update(encrypted_data);
        
        // Canonicalize metadata by serializing sorted keys
        let mut sorted_metadata: Vec<_> = metadata.iter().collect();
        sorted_metadata.sort_by_key(|(k, _)| *k);
        
        for (key, value) in sorted_metadata {
            hasher.update(key.as_bytes());
            hasher.update(
                serde_json::to_string(value)
                    .map_err(|e| GhostError::Serialization(e.to_string()))?
                    .as_bytes(),
            );
        }
        
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }
    
    /// Validate CID matches the content
    pub fn validate_cid(&self) -> GhostResult<bool> {
        let calculated = Self::generate_cid(&self.encrypted_data, &self.metadata)?;
        Ok(calculated == self.id)
    }
    
    /// Serialize to JSON for harvester output
    pub fn to_json(&self) -> GhostResult<String> {
        serde_json::to_string(self).map_err(|e| GhostError::Serialization(e.to_string()))
    }
    
    /// Deserialize from JSON (from harvester output)
    pub fn from_json(json_str: &str) -> GhostResult<Self> {
        serde_json::from_str(json_str).map_err(|e| GhostError::Serialization(e.to_string()))
    }
}

/// Plugin trait for extensible encryption/decryption
pub trait EncryptionPlugin: Send + Sync {
    fn encrypt(&self, data: &[u8]) -> GhostResult<Vec<u8>>;
    fn decrypt(&self, encrypted: &[u8]) -> GhostResult<Vec<u8>>;
    fn name(&self) -> &str;
}

/// Storage backend trait
pub trait StoragePlugin: Send + Sync {
    fn store(&self, cid: &str, data: &[u8]) -> GhostResult<()>;
    fn retrieve(&self, cid: &str) -> GhostResult<Vec<u8>>;
    fn name(&self) -> &str;
}

/// Harvester interface definition
pub trait Harvester: Send + Sync {
    fn harvest(&self) -> GhostResult<Vec<StateObject>>;
    fn name(&self) -> &str;
}

/// Core configuration for the Ghost engine
#[derive(Serialize, Deserialize, Clone)]
pub struct GhostConfig {
    pub firebase_project_id: String,
    pub s3_bucket: String,
    pub aws_region: String,
    pub local_cache_path: String,
    pub harvest_interval_seconds: u64,
    pub enabled_harvesters: Vec<String>,
}

impl Default for GhostConfig {
    fn default() -> Self {
        Self {
            firebase_project_id: "ghost-memory".to_string(),
            s3_bucket: "ghost-state-backup".to_string(),
            aws_region: "us-east-1".to_string(),
            local_cache_path: "~/.ghost/cache".to_string(),
            harvest_interval_seconds: 300, // 5 minutes
            enabled_harvesters: vec![
                "filesystem".to_string(),
                "process".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_object_cid_generation() {
        let mut metadata = HashMap::new();
        metadata.insert("mime_type".to_string(), serde_json::Value::String("text/plain".to_string()));
        metadata.insert("size".to_string(), serde_json::Value::Number(serde_json::Number::from(42)));
        
        let data = b"test data for CID generation";
        
        let state_obj = StateObject::new(
            "test:source".to_string(),
            data.to_vec(),
            metadata,
        ).unwrap();
        
        assert!(!state_obj.id.is_empty());
        assert_eq!(state_obj.id.len(), 64); // SHA256 hex length
        assert!(state_obj.validate_cid().unwrap());
    }
    
    #[test]
    fn test_serialization_roundtrip() {
        let mut metadata = HashMap::new();
        metadata.insert("test_key".to_string(), serde_json::Value::Bool(true));
        
        let original = StateObject::new(
            "test".to_string(),
            vec![1, 2, 3, 4, 5],
            metadata,
        ).unwrap();
        
        let json = original.to_json().unwrap();
        let deserialized = StateObject::from_json(&json).unwrap();
        
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.source, deserialized.source);
        assert_eq!(original.encrypted_data, deserialized.encrypted_data);
    }
}