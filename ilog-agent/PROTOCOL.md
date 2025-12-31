# iLog Agent TCP Protocol Specification

## Overview

The iLog agent uses a custom TCP protocol optimized for low-latency log streaming with built-in encryption and compression.

## Protocol Stack

```
┌─────────────────────────────────┐
│   Application (JSON/OTLP)       │
├─────────────────────────────────┤
│   LZ4 Compression (2-3x)        │
├─────────────────────────────────┤
│   ChaCha20-Poly1305 Encryption  │
├─────────────────────────────────┤
│   Frame Protocol                │
├─────────────────────────────────┤
│   TCP Socket                    │
└─────────────────────────────────┘
```

## Frame Format

Each frame consists of:

```
┌──────────┬─────────┬────────────┬────────────┬─────────────┐
│  Magic   │ Version │ Frame Type │   Length   │   Payload   │
│ 4 bytes  │ 1 byte  │   1 byte   │  4 bytes   │  N bytes    │
└──────────┴─────────┴────────────┴────────────┴─────────────┘
```

### Header Fields

- **Magic Bytes**: `ILOG` (0x494C4F47) - Protocol identifier
- **Version**: `0x01` - Protocol version
- **Frame Type**: 
  - `0x01` - Log Batch
  - `0x02` - Heartbeat
  - `0x03` - Ack
- **Length**: 32-bit unsigned integer (big-endian) - Payload size in bytes
- **Payload**: Encrypted and compressed data

### Maximum Frame Size

- Max payload: 100 MB (configurable)
- Typical batch: 10-100 KB compressed

## Encryption

**Algorithm**: ChaCha20-Poly1305 (AEAD)

### Key Derivation

The encryption key is derived from the project token:
```rust
key = hash(token) -> [u8; 32]
```

### Encryption Process

1. Generate random 12-byte nonce
2. Encrypt plaintext with ChaCha20-Poly1305
3. Prepend nonce to ciphertext
4. Result: `[nonce (12 bytes)][ciphertext + tag (N + 16 bytes)]`

### Security Properties

- ✅ Authenticated encryption (prevents tampering)
- ✅ Unique nonce per message (prevents replay)
- ✅ 256-bit key strength
- ✅ Constant-time operations

## Compression

**Algorithm**: LZ4 Block Compression

### Compression Settings

- Mode: Fast compression (no dictionary)
- Typical ratio: 2-3x for JSON logs
- Latency: <1ms for typical batches

### Why LZ4?

| Metric | LZ4 | zstd | zstd+dict |
|--------|-----|------|-----------|
| Speed | ⚡⚡⚡ | ⚡⚡ | ⚡ |
| Ratio | 2-3x | 2.5-4x | 3-5x |
| Latency | <1ms | 1-3ms | 2-5ms |
| Memory | Low | Medium | High |

LZ4 wins for real-time log streaming due to minimal latency.

## Data Flow

### Agent → Server

```
1. Collect logs → LogEntry[]
2. Serialize to JSON (OTLP format)
3. Compress with LZ4
4. Encrypt with ChaCha20-Poly1305
5. Frame with protocol header
6. Send over TCP socket
```

### Example Frame (Log Batch)

```
Hex dump:
494C 4F47 01 01 0000 0234 [encrypted payload...]
│    │    │  │  │         │
│    │    │  │  │         └─ Encrypted + compressed logs
│    │    │  │  └─────────── Length: 564 bytes
│    │    │  └────────────── Frame type: Log Batch (0x01)
│    │    └───────────────── Version: 1
│    └────────────────────── Magic: ILOG
```

## Connection Management

### Connection Lifecycle

1. **Connect**: TCP handshake to server:port
2. **Authenticate**: First frame contains token (in encrypted payload)
3. **Stream**: Send log batches as they accumulate
4. **Heartbeat**: Send heartbeat every 30s if idle
5. **Reconnect**: Exponential backoff on disconnect (1s, 2s, 4s)

### TCP Settings

- `TCP_NODELAY`: Enabled (disable Nagle's algorithm)
- Keep-alive: System default
- Buffer size: System default

### Error Handling

- Connection lost → Buffer logs in memory (up to channel capacity)
- Max retries: 3 attempts with exponential backoff
- On failure → Log error, continue collecting

## Performance Characteristics

### Latency Breakdown

```
Log event → Agent processing:     <0.1ms
Serialization (JSON):             ~0.2ms
Compression (LZ4):                ~0.5ms
Encryption (ChaCha20):            ~0.3ms
TCP send (local network):         ~0.5ms
──────────────────────────────────────────
Total (typical):                  ~1.6ms
```

### Throughput

- Single connection: ~50,000 logs/sec
- Micro-batching: 10ms window (collects burst logs)
- Max micro-batch: 50 logs (network efficiency)
- Real-time: Logs sent immediately after micro-batch window

### Memory Usage

- Channel buffer: 1000 log entries
- Typical log entry: ~500 bytes
- Total buffer: ~500 KB
- Agent overhead: ~5-10 MB

### Real-time Streaming

The agent uses **micro-batching** for optimal performance:

1. **Log arrives** → Start 10ms timer
2. **Collect burst** → Grab any additional logs in queue (max 50)
3. **Send immediately** → No artificial delays
4. **Result**: ~10-15ms latency from log event to server

This balances real-time delivery with network efficiency (avoiding 1 TCP packet per log).

## Server Implementation Requirements

To receive logs from the agent, the server must:

1. **Listen on TCP port** (e.g., 8080)
2. **Read frames** using the protocol format
3. **Decrypt payload** with ChaCha20-Poly1305 (key from token)
4. **Decompress** with LZ4
5. **Parse JSON** (OTLP format)
6. **Process logs** (store, index, alert, etc.)

### Example Server Pseudocode

```rust
loop {
    let frame = Frame::read_from(&mut stream).await?;
    
    match frame.frame_type {
        FrameType::LogBatch => {
            let decrypted = encryptor.decrypt(&frame.payload)?;
            let decompressed = lz4::decompress(&decrypted)?;
            let logs: Vec<LogEntry> = serde_json::from_slice(&decompressed)?;
            process_logs(logs).await?;
        }
        FrameType::Heartbeat => {
            // Optional: send ack
        }
        _ => {}
    }
}
```

## Comparison with HTTP

| Metric | TCP Protocol | HTTP/1.1 |
|--------|-------------|----------|
| Latency | ~1-2ms | ~5-15ms |
| Connection | Persistent | Per-request |
| Overhead | 10 bytes | ~200+ bytes |
| Encryption | ChaCha20 | TLS |
| Compression | LZ4 | gzip/none |
| Firewall | May need rules | Standard |

## Future Enhancements

- [ ] TLS wrapper option (TCP + TLS)
- [ ] Multiplexing (multiple streams per connection)
- [ ] Flow control (backpressure signaling)
- [ ] Compression dictionary support
- [ ] Binary protocol (MessagePack/Protobuf)
