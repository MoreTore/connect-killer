#!/bin/bash
CONTAINER=connect-killer-connect-1
# copy over frontend files
docker cp frontend/. $CONTAINER:/usr/src/connect/frontend/
# restart the connect service
docker exec $CONTAINER pkill -9 -f "connect" 2>/dev/null || true
# 
docker exec -d $CONTAINER /bin/bash -c "cd /usr/src/connect && ./start_connect.sh & ./start_useradmin.sh"
