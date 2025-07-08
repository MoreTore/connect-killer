#!/bin/bash
CONTAINER=connect-killer-connect-1
docker cp assets/. $CONTAINER:/usr/src/connect/assets/
docker cp ~/connect-killer/config/. $CONTAINER:/usr/src/connect/config/
docker exec $CONTAINER pkill -9 -f "connect" 2>/dev/null || true
docker exec -d $CONTAINER /bin/bash -c "cd /usr/src/connect && ./start_connect.sh & ./start_useradmin.sh"