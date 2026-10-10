#!/bin/bash
TARGET_DIR="$HOME/akira-frontier"
REMOTE="aqua:AkiraBackup/chunks"

while true; do
    # যদি found.jsonl এর সাইজ ৫০০ MB ছাড়িয়ে যায়
    FILESIZE=$(stat -c%s "$TARGET_DIR/found.jsonl" 2>/dev/null || echo 0)
    if [ "$FILESIZE" -gt 524288000 ]; then
        TIMESTAMP=$(date +%Y%m%d_%H%M%S)
        CHUNK_NAME="chunk_${TIMESTAMP}.jsonl"
        
        # ফাইল মুভ করে রিনেম করা (যাতে ইঞ্জিন সাথে সাথে নতুন ফাইল শুরু করতে পারে)
        mv "$TARGET_DIR/found.jsonl" "$TARGET_DIR/$CHUNK_NAME"
        
        # ড্রাইভে পাঠানো এবং সাথে সাথে ফোন থেকে ডিলিট করা
        rclone move "$TARGET_DIR/$CHUNK_NAME" "$REMOTE/" --drive-chunk-size 64M &
    fi
    sleep 60
done
