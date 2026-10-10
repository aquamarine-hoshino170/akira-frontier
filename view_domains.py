import json
from collections import defaultdict

counts = defaultdict(lambda: {"total": 0, "chaos": 0})
total_records = 0

print("ডেটাসেট স্ক্যান করা হচ্ছে (২ লাখ+ রেকর্ড)...")

with open("found.jsonl", "r", encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        try:
            data = json.loads(line)
            # সঠিক ফিল্ড 'family' থেকে বিষয়ের নাম নেওয়া
            dom = data.get("family", "unknown")
            
            # ক্যাওস যাচাই: status বা evidence.behavior
            status = data.get("status", "")
            behavior = data.get("evidence", {}).get("behavior", "")
            is_chaos = "chaotic" in status or behavior == "chaotic"
            
            counts[dom]["total"] += 1
            if is_chaos:
                counts[dom]["chaos"] += 1
            total_records += 1
        except Exception:
            continue

print("=" * 75)
print(f"AKIRA-FRONTIER: মোট রেকর্ড: {total_records} | মোট সক্রিয় বিষয়: {len(counts)}")
print("=" * 75)
print(f"{'বিষয় (Family / Domain)':<32} | {'মোট':<8} | {'ক্যাওটিক':<8} | {'ক্যাওস হার'}")
print("-" * 75)

for dom, stat in sorted(counts.items(), key=lambda x: x[1]["total"], reverse=True):
    tot = stat["total"]
    ch = stat["chaos"]
    rate = (ch / tot * 100) if tot > 0 else 0.0
    print(f"{dom:<32} | {tot:<8} | {ch:<8} | {rate:6.1f}%")

print("=" * 75)
