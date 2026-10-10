with open("src/main.rs", "r", encoding="utf-8") as f:
    text = f.read()

# ১. ভুলভাবে নেস্টেড হওয়া _ => { let (f, s, p, dom_name) প্যাটার্ন সংশোধন
bad_pattern = "_ => { let (f, s, p, dom_name) = match rng.gen_range(0..115) {"
good_pattern = "let (f, s, p, dom_name) = match rng.gen_range(0..115) {"

if bad_pattern in text:
    text = text.replace(bad_pattern, good_pattern)
    print("Fixed nested match block syntax!")

# ২. অপ্রয়োজনীয় ডুপ্লিকেট gen_extended_100_domain থাকলে একটি রেখে বাকি পরিষ্কার
parts = text.split("pub fn gen_extended_100_domain")
if len(parts) > 2:
    # যদি একাধিকবার যুক্ত হয়ে থাকে, প্রথমটি বাদে বাকিগুলো সরানো
    text = parts[0] + "pub fn gen_extended_100_domain" + parts[1]
    print("Removed duplicate function definitions!")

with open("src/main.rs", "w", encoding="utf-8") as f:
    f.write(text)

print("DELIMITER FIXED!")
