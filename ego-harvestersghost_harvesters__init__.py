"""
Ghost Harvesters - Pluggable state extraction modules for the Ghost engine

This package contains various harvesters that extract state from different sources
and output StateObject JSON for consumption by the ego-core Rust kernel.
"""

import sys
import json
import logging
from typing import Dict, List, Any, Optional
from abc import ABC, abstractmethod
from pathlib import Path

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

__version__ = "0.1.0"
__all__ = ["BaseHarvester", "StateObject", "harvest_from_cli"]


class StateObject:
    """Python representation of the StateObject protocol"""
    
    def __init__(
        self,
        id: str,
        timestamp: str,
        source: str,
        encrypted_data: bytes,
        metadata: Dict[str, Any],
        signature: Optional[bytes] = None
    ):
        self.id = id
        self.timestamp = timestamp
        self.source = source
        self.encrypted_data = encrypted_data
        self.metadata = metadata
        self.signature = signature
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization"""
        return {
            "id": self.id,
            "timestamp": self.timestamp,
            "source": self.source,
            "encrypted_data": self.encrypted_data.hex(),
            "metadata": self.metadata,
            "signature": self.signature.hex() if self.signature else None
        }
    
    def to_json(self) -> str:
        """Serialize to JSON string"""
        return json.dumps(self.to_dict())
    
    @classmethod
    def from_json(cls, json_str: str) -> "StateObject":
        """Deserialize from JSON string"""
        data = json.loads(json_str)
        return cls(
            id=data["id"],
            timestamp=data["timestamp"],
            source=data["source"],
            encrypted_data=bytes.fromhex(data["encrypted_data"]),
            metadata=data["metadata"],
            signature=bytes.fromhex(data["signature"]) if data.get("signature") else None