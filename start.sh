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
echo "Log created on $(date +'%Y-%m-%d %H:%M:%S')" | tee "$LOGFILE"
echo "System Information:" | tee -a "$LOGFILE"
echo "Hostname: $(hostname)" | tee -a "$LOGFILE"
echo "Operating System: $(uname -a)" | tee -a "$LOGFILE"
echo "-----------------------------------------" | tee -a "$LOGFILE"
docker run -d -p 3000-3005:3000-3005 -v kvstore:/tmp minikeyvalue
docker run -d -p 5432:5432 -e POSTGRES_USER=loco -e POSTGRES_DB=connect_development -e POSTGRES_PASSWORD="loco" -v pgdata:/var/lib/postgresql/data --name pg_connect postgres:15.3-alpine 
cd pgvector
# Copy vector.so to the /usr/local/lib/postgresql/ directory inside the container
docker cp ./pgvector/vector.so pg_connect:/usr/local/lib/postgresql/vector.so
# Copy vector.control to the /usr/local/share/postgresql/extension/ directory inside the container
docker cp ./pgvector/vector.control pg_connect:/usr/local/share/postgresql/extension/vector.control
docker cp ./pgvector/sql/vector--0.4.4.sql pg_connect:/usr/local/share/postgresql/extension


docker run -p 6379:6379 -d redis redis-server
