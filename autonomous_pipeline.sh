#!/bin/bash
DIR="$HOME/akira-frontier"
REMOTE="aqua:AkiraBackup/chunks"
THRESHOLD=104857600  # 100 MB

mkdir -p "$DIR/staging"

while true; do
    if [ -f "$DIR/found.jsonl" ]; then
        SIZE=$(stat -c%s "$DIR/found.jsonl" 2>/dev/null || echo 0)
        
        if [ "$SIZE" -ge "$THRESHOLD" ]; then
            TIMESTAMP=$(date +%Y%m%d_%H%M%S)
            SNAP="$DIR/staging/snap_${TIMESTAMP}.jsonl"
            
            mv "$DIR/found.jsonl" "$SNAP"
            touch "$DIR/found.jsonl"
            
            tar -cf - -C "$DIR/staging" "snap_${TIMESTAMP}.jsonl" | zstd -19 -o "$DIR/staging/chunk_${TIMESTAMP}.tar.zst"
            rm "$SNAP"
            
            rclone move "$DIR/staging/chunk_${TIMESTAMP}.tar.zst" "$REMOTE/" --drive-chunk-size 64M
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] [AUTONOMOUS] Chunk ${TIMESTAMP} pushed to Drive."
        fi
    fi
    sleep 30
done
