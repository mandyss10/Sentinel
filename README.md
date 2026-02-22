# 🛡️ Sentinel: The AI Performance Firewall

**Sentinel** is a high-performance proxy built in Rust designed to prevent AI agents from entering infinite semantic loops, leaking sensitive data, or causing runaway costs ("Math of Ruin").

## 🚀 Core Features

- **Performance-First Proxy**: Built with **Rust (Axum)** for sub-10ms latency.
- **Multi-Provider Support**: Seamlessly route requests between **OpenAI** and **Groq** for ultra-fast inference.
- **Semantic Loop Detection**: Guards against "semantic stalls" by analyzing embeddings distance.
- **Anti-Exploit Security**: 
    - **Economic Throttling**: Automatically cuts execution if spending grows exponentially.
    - **EchoLeak Protection**: Filters patterns used in indirect injections to exfiltrate data.
- **MCP Tool Provider**: Direct integration with Cursor, Claude Code, and VS Code via the Model Context Protocol.

## 💰 Business Value: ROI Calculator

Sentinel isn't just a firewall; it's a cost-saving engine. Use the formula below to calculate the ROI for your enterprise:

$$ROI = \frac{(H_{manual} \times R_{hora}) - (C_{tokens} + C_{Sentinel})}{C_{Sentinel}} \times 100$$

*On average, Sentinel reduces token waste by **40%-60%** by eliminating redundant loops and runaway tasks.*

## 🎬 Sentinel in Action: Proof of Concept

Don't just take our word for it. Sentinel includes a **Sales-Ready Pitch Demo** that simulates real agent failures and demonstrates immediate ROI.

```bash
# Terminal 1: Start Sentinel
cargo run

# Terminal 2: Run the High-Impact Demo
python scripts/demo_pitch.py
```

### What happens?
1. **Agent Failure**: We simulate an agent stuck in a "System Log Analysis" loop.
2. **Sentinel Interception**: Sentinel detects the semantic repetition in <10ms.
3. **ROI Report**: The demo calculates the **Dollars Saved** by preventing the runaway execution.

| Attack Vector | Before Sentinel | After Sentinel | Status |
| :--- | :--- | :--- | :--- |
| **Semantic Loop** | $50.00+ Waste | $0.50 Detection | ✅ Blocked |
| **Token Runaway** | Unlimited Spend | Custom Caps | ✅ Throttled |
| **Data Leak** | Privacy Breach | Real-time Filter | ✅ Secure |

## 🛠️ Tech Stack

- **Language**: Rust
- **Framework**: Axum 0.8 / Tokio
- **State Management**: DashMap (Concurrent Hash Table)
- **Token Analysis**: Semantic Cosine Similarity (OpenAI Embeddings)
- **Interface**: HTTP Proxy + MCP (JSON-RPC)

## 📦 Setup & Installation

1. Create a `.env` file with your `OPENAI_API_KEY`.
2. Run with Cargo:
   ```bash
   cargo run
   ```
3. Target Sentinel in your LLM configuration: `http://localhost:3000/v1`.

## ⚖️ Licensing

Sentinel is licensed under the **AGPL-3.0**. 
- **Community**: Free for personal and startup use.
- **Enterprise**: Commercial licenses available for SaaS integration without source disclosure.
