# 🔄 Sentinel Minimal Reproducible Example

This script demonstrates how Sentinel detects a semantic loop and intercepts it.

## 1. Prerequisites
- Sentinel running on `http://localhost:3000`
- `OPENAI_API_KEY` set in Sentinel's `.env`

## 2. Reproduction Script (`reproduce_loop.py`)

```python
import openai

# Configure the client to point to Sentinel
client = openai.OpenAI(
    base_url="http://localhost:3000/v1",
    api_key="your-openai-api-key" 
)

session_id = "test-loop-session-123"

print("🚀 Starting agent loop simulation...\n")

for i in range(5):
    print(f"Turn {i+1}: Sending repetitive request...")
    
    # We send slightly different prompts to simulate an agent 'stuck' but with stochastic variance
    response = client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {"role": "system", "content": "Keep repeating that you are searching the database."},
            {"role": "user", "content": "What are you doing?"}
        ],
        extra_headers={"x-sentinel-session": session_id}
    )
    
    content = response.choices[0].message.content
    print(f"Agent Response: {content}")
    
    if "SENTINEL" in content:
        print("\n✅ SUCCESS: Sentinel intercepted the loop!")
        break
```

## 3. Expected Output

```text
Turn 1: Sending repetitive request...
Agent Response: I am currently searching the database for your information.
Turn 2: Sending repetitive request...
Agent Response: I'm still in the process of searching the database.
Turn 3: Sending repetitive request...
Agent Response: 🚨 SENTINEL: Detectado bucle semántico. Cambia tu estrategia o pide ayuda.

✅ SUCCESS: Sentinel intercepted the loop!
```
