#!/bin/bash
while true; do
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Starting sync to Google Drive..."
    rclone copy ~/akira-frontier/found.jsonl aqua:AkiraBackup/ --drive-chunk-size 64M
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] Backup complete! Waiting 30 minutes..."
    sleep 1800
done
