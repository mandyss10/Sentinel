import requests
import time
import sys

# Terminal Colors
BLUE = "\033[94m"
GREEN = "\033[92m"
YELLOW = "\033[93m"
RED = "\033[91m"
BOLD = "\033[1m"
RESET = "\033[0m"

SENTINEL_URL = "http://localhost:3000/v1/chat/completions"

def print_banner():
    print(f"{BOLD}{BLUE}" + "="*50)
    print("      🛡️  SENTINEL: AI PERFORMANCE FIREWALL")
    print("="*50 + f"{RESET}\n")

def simulate_loop():
    print_banner()
    session_id = f"pitch-demo-{int(time.time())}"
    
    # Intentional loop messages
    messages = [
        "Analyzing system logs to find the root cause...",
        "Still analyzing the same logs for patterns...",
        "Re-checking logs for the third time to be sure...",
        "Initiating final log analysis pass..."
    ]

    total_waste_prevented = 0.0
    
    print(f"{BOLD}Step 1: Simulating an AI Agent going into a loop...{RESET}")
    print(f"Targeting Sentinel Proxy at: {SENTINEL_URL}\n")

    for i, msg in enumerate(messages):
        time.sleep(1)
        print(f"{YELLOW}[Agent]{RESET} Attempting Action {i+1}: {msg}")
        
        try:
            payload = {
                "model": "llama-3.3-70b-versatile", # Groq-powered for fast demo
                "messages": [{"role": "user", "content": msg}],
            }
            headers = {"x-sentinel-session": session_id}
            
            start = time.time()
            response = requests.post(SENTINEL_URL, json=payload, headers=headers)
            latency = (time.time() - start) * 1000
            
            data = response.json()
            content = data['choices'][0]['message']['content']

            if "SENTINEL" in content:
                print(f"\n{BOLD}{RED}[!!!] SENTINEL INTERVENTION DETECTED [!!!]{RESET}")
                print(f"{RED}Reason:{RESET} {content}")
                print(f"{GREEN}Latency Overhead:{RESET} {latency:.2f}ms")
                
                # Business logic: Every loop blocked saves roughly $0.05 - $0.50 in compute/tokens
                total_waste_prevented += 0.50 
                
                print(f"\n{BOLD}{GREEN}💰 SAVINGS DEMONSTRATED:{RESET}")
                print(f"Prevented Runaway Cost: ${total_waste_prevented:.2f}")
                print(f"Status: {BOLD}Thread Terminated Safely.{RESET}")
                print(f"\n{BLUE}" + "="*50 + f"{RESET}")
                return
            else:
                print(f"{BLUE}[Sentinel]{RESET} Request passing... (Latency: {latency:.2f}ms)")
                
        except Exception as e:
            print(f"{RED}Error connecting to Sentinel. Make sure it's running!{RESET}")
            return

    print(f"\n{RED}❌ Loop not intercepted. Check sensitivity settings.{RESET}")

if __name__ == "__main__":
    simulate_loop()
