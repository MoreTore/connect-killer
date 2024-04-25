#!/bin/bash
# Generate a date stamp for filename
DATE=$(date +'%Y-%m-%d %H:%M:%S')
LOGFILE="log/logfile_$DATE.txt"

adddate() {
    while IFS= read -r line; do
        printf '%s %s\n' "$(date)" "$line";
    done
}

# Create log file and insert header
# echo "Log created on $(date +'%Y-%m-%d %H:%M:%S')" | tee "$LOGFILE"
# echo "System Information:" | tee -a "$LOGFILE"
# echo "Hostname: $(hostname)" | tee -a "$LOGFILE"
# echo "Operating System: $(uname -a)" | tee -a "$LOGFILE"
# echo "-----------------------------------------" | tee -a "$LOGFILE"

sudo bash setup.sh | adddate >>"$LOGFILE"