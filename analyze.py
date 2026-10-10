import json
from collections import Counter

all_domains = {
    "lienard-2D": "ক্লাসিক্যাল নন-লিনিয়ার মেকানিক্স",
    "sprott-3D": "অ্যাবস্ট্রাক্ট ৩ডি পলিনমিয়াল স্পেস",
    "autonomous-1D": "বিশুদ্ধ ১ডি ট্রান্সেডেন্টাল বাউন্ডারি",
    "oscillator": "ডাইমেনশনাল স্যাচুরেশন মেকানিক্স",
    "biology-FHN": "নিউরোনাল এক্সাইট্যাবিলিটি (২ডি)",
    "chem-brusselator": "অটোক্যাটালাইটিক কেমিক্যাল ক্লক (২ডি)",
    "cosmo-quintessence": "এফএলআরডব্লিউ ডার্ক এনার্জি ফেজ-স্পেস",
    "quantum-semiclass": "সেমিক্ল্যাসিক্যাল ড্রাইভেন কোয়ান্টাম ফ্লো",
    "chem-oregonator-3D": "বিজেড কেমিক্যাল ৩ডি রিঅ্যাকশন",
    "bio-foodweb-3D": "হেস্টিংস-পাওয়েল ৩-প্রজাতির ফুড-ওয়েব",
    "fluid-navier-stokes": "নেভিয়ার-স্টোকস থার্মাল কনভেকশন",
    "em-maxwell-bloch": "ম্যাক্সওয়েল-ব্লচ রেজোন্যান্স",
    "gr-mixmaster": "বিয়াঙ্কি ৯ মিক্সমাস্টার গ্র্যাভিটি",
    "thermo-onsager": "অনসাগার ডিসিপেটিভ ইঞ্জিন",
    "gauge-yang-mills": "ইয়াং-মিলস এসইউ(২) গেজ ফিল্ড"
}

reasons = {
    "quantum-semiclass": "এক্সটার্নাল ড্রাইভ ফ্রিকোয়েন্সির কারণে ট্র্যাজেক্টরি বাউন্ডেড রিজিয়ন ছাড়িয়ে দ্রুত আনবাউন্ডেড হয়ে বাদ পড়ছে।",
    "chem-oregonator-3D": "স্টিফ ওডিই (Stiff ODE) হওয়ায় ক্ষুদ্র টাইমস্কেলের জন্য সাধারণ আরকে-৪ এ ট্র্যাজেক্টরি ডাইভারজেন্স ফিল্টারে আটকে যাচ্ছে।",
    "bio-foodweb-3D": "প্যারামিটার স্পেস সংকীর্ণ হওয়ায় বাউন্ডেড ক্যাওটিক উইন্ডো সহজে হিট করছে না।",
    "fluid-navier-stokes": "আগের কম্পাইলেশন সিগ-টার্ম (SIGTERM) পাওয়ায় নতুন বাইনারি মেমোরিতে লোড হয়নি, পুরোনো কোড চলছে।",
    "em-maxwell-bloch": "নতুন বাইনারি কম্পাইল না হয়ে আগের এক্সিকিউটেবল ব্যাকগ্রাউন্ডে চলছে।",
    "gr-mixmaster": "নতুন বাইনারি কম্পাইল না হয়ে আগের এক্সিকিউটেবল ব্যাকগ্রাউন্ডে চলছে।",
    "thermo-onsager": "নতুন বাইনারি কম্পাইল না হয়ে আগের এক্সিকিউটেবল ব্যাকগ্রাউন্ডে চলছে।",
    "gauge-yang-mills": "নতুন বাইনারি কম্পাইল না হয়ে আগের এক্সিকিউটেবল ব্যাকগ্রাউন্ডে চলছে।"
}

found_counts = Counter()
chaotic_counts = Counter()
total_records = 0

with open("found.jsonl", "r") as f:
    for line in f:
        total_records += 1
        d = json.loads(line)
        fam = d.get("family", "unknown")
        found_counts[fam] += 1
        if d.get("evidence", {}).get("behavior") == "chaotic":
            chaotic_counts[fam] += 1

print("\n" + "=" * 70)
print(f"AKIRA-FRONTIER ডেটাসেট রিপোর্ট (মোট সংগৃহীত রেকর্ড: {total_records})")
print("=" * 70)
print(f"{'বিষয় (Domain)':<22} | {'মোট':<6} | {'ক্যাওটিক':<8} | {'স্ট্যাটাস'}")
print("-" * 70)

missing = []

for fam, desc in all_domains.items():
    count = found_counts[fam]
    chaos = chaotic_counts[fam]
    if count > 0:
        pct = (chaos / count) * 100
        print(f"{fam:<22} | {count:<6} | {chaos:<8} | সফল ({pct:.1f}% ক্যাওস)")
    else:
        print(f"{fam:<22} | {0:<6} | {0:<8} | শূন্য (০)")
        missing.append(fam)

print("-" * 70)

if missing:
    print("\n[যে বিষয়গুলো এখনো জমা পড়েনি এবং তার কারণ]:")
    for fam in missing:
        r = reasons.get(fam, "সিস্টেম জেনারেটর এখনও সফলভাবে এক্সিকিউট হয়নি।")
        print(f"* {fam}: {r}")

print("=" * 70)
