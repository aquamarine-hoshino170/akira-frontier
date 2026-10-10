with open("src/main.rs", "r", encoding="utf-8") as f:
    text = f.read()

# ১০০ ডোমেইনের ফাংশনটির ঠিক আগের অংশ আলাদা করা
parts = text.split("pub fn gen_extended_100_domain")

if len(parts) >= 2:
    main_code = parts[0]
    extended_code = "pub fn gen_extended_100_domain" + "pub fn gen_extended_100_domain".join(parts[1:])
    
    # মূল কোডে কতগুলো { এবং } আছে তা গণা
    open_count = main_code.count('{')
    close_count = main_code.count('}')
    
    # যদি শেষের ব্র্যাকেট মুছে গিয়ে থাকে, তবে তা রিস্টোর করা
    if open_count > close_count:
        missing_brackets = open_count - close_count
        main_code = main_code.rstrip() + "\n" + ("}\n" * missing_brackets)
        print(f"🔥 ULTIMATE FIX: {missing_brackets} missing closing brackets ( '}}' ) magically restored!")
    elif open_count == close_count:
        print("Brackets are already perfectly balanced!")
    else:
        print("Warning: More closing brackets than opening ones.")

    # ফাইলটি নতুন করে নিখুঁতভাবে জোড়া লাগানো
    with open("src/main.rs", "w", encoding="utf-8") as f:
        f.write(main_code + "\n\n" + extended_code)
else:
    print("Error: Could not find the extended domains function.")
