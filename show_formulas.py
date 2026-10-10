import json

domains = [
    "fluid-navier-stokes",
    "em-maxwell-bloch",
    "gr-mixmaster",
    "thermo-onsager",
    "gauge-yang-mills",
    "bio-foodweb-3D",
    "chem-oregonator-3D",
    "chem-brusselator",
    "biology-FHN",
    "cosmo-quintessence"
]

best_systems = {}

with open("found.jsonl", "r") as f:
    for line in f:
        d = json.loads(line)
        fam = d.get("family")
        if fam in domains:
            ev = d.get("evidence", {})
            lyap = ev.get("lyapunov_max", 0.0)
            beh = ev.get("behavior", "")
            
            if fam not in best_systems or lyap > best_systems[fam]["lyap"]:
                best_systems[fam] = {
                    "lyap": lyap,
                    "behavior": beh,
                    "hash": d.get("hash", ""),
                    "eqs": d.get("equations", [])
                }

print("=" * 75)
print("     আবিষ্কৃত মৌলিক ক্ষেত্রসমূহের শীর্ষ সমীকরণসমূহ (Top Dynamic Formulas)     ")
print("=" * 75)

for fam in domains:
    if fam in best_systems:
        info = best_systems[fam]
        beh = info["behavior"]
        lyap = info["lyap"]
        h = info["hash"]
        print(f"\n[ডোমেইন: {fam}]")
        print(f"আচরণ: {beh} | λ_max = {lyap:.4f} | Hash: {h}")
        print("সমীকরণমালা:")
        for eq in info["eqs"]:
            print(f"   • {eq}")
        print("-" * 75)
