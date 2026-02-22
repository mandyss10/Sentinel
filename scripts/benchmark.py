import time
import requests
import statistics
import os

# --- CONFIG ---
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")
SENTINEL_URL = "http://localhost:3000/v1/chat/completions"
DIRECT_URL = "https://api.openai.com/v1/chat/completions"
NUM_TESTS = 5

payload = {
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": "Say 'Latency Test'"}],
}

headers = {
    "Authorization": f"Bearer {OPENAI_API_KEY}",
    "Content-Type": "application/json"
}

def measure_call(url):
    start = time.perf_counter()
    try:
        response = requests.post(url, json=payload, headers=headers, timeout=30)
        response.raise_for_status()
    except Exception as e:
        print(f"Error calling {url}: {e}")
        return None
    return (time.perf_counter() - start) * 1000  # ms

print(f"🔬 Starting latency benchmark ({NUM_TESTS} calls each)...")

direct_times = []
proxy_times = []

for i in range(NUM_TESTS):
    # Direct
    t_direct = measure_call(DIRECT_URL)
    if t_direct: direct_times.append(t_direct)
    
    # Proxy
    t_proxy = measure_call(SENTINEL_URL)
    if t_proxy: proxy_times.append(t_proxy)
    
    print(f"Step {i+1}: Direct={t_direct:.1f}ms | Proxy={t_proxy:.1f}ms")

if direct_times and proxy_times:
    avg_direct = statistics.mean(direct_times)
    avg_proxy = statistics.mean(proxy_times)
    overhead = avg_proxy - avg_direct
    
    print("\n--- RESULTS ---")
    print(f"Average Direct Latency: {avg_direct:.2f}ms")
    print(f"Average Proxy Latency:  {avg_proxy:.2f}ms")
    print(f"Sentinel Overhead:      {overhead:.2f}ms")
    print(f"p95 Proxy Latency:      {statistics.quantiles(proxy_times, n=20)[18]:.2f}ms")
    
    if overhead < 20:
        print("\n✅ Performance is within expected production limits (<20ms overhead).")
else:
    print("\n❌ Benchmark failed due to API errors.")
