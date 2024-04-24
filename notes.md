# user relationships
    user can be owner to many devices
    user can be authorized user to many devices

# device relationships
    - device has many routes
    - has one owner
    - has many authorized users

# route relationships
    - route has many segments
    - route belongs to device
    - route can be public or private (default is private)

# segment relationships
    - segment belongs to route
    - segment can have one of each file: qlog, qcamera, rlog, dcamera, fcamera, ecamera

# Entity-Relationship Diagram
User
├── Owned Devices (One-to-Many)
│    └── Device
│         ├── Owned by User (Many-to-One)
│         ├── Authorized Users (Many-to-Many)
│         └── Routes (One-to-Many)
│              └── Route
│                   ├── Belongs to Device (Many-to-One)
│                   ├── Visibility (Attribute: public/private)
│                   └── Segments (One-to-Many)
│                        └── Segment
│                             ├── Belongs to Route (Many-to-One)
│                             └── Files (Attributes: qlog, qcamera, rlog, dcamera, fcamera, ecamera)
└── Authorized Devices (Many-to-Many)


# segment model
"git_remote"	        string	Git remote from openpilot log InitData
"start_time_utc_millis"	integer	Milliseconds since epoch of segment start time, from GPS
"number"	            integer	Segment number
"proc_dcamera"	        integer	Driver camera file status. See Segment File Status below
"radar"	                boolean	 True if segment contains radar tracks in CAN
"create_time"	        integer	time of upload_url call for first file uploaded of segment
"hpgps"	                boolean	True if segment has ublox packets
"end_time_utc_millis"	integer	Milliseconds since epoch of segment end time, from GPS
"end_lng"	            float	Last longitude recorded in segment, from GPS
"start_lng"	            float	First longitude recorded in segment, from GPS
"passive"	            boolean	True if openpilot was running in passive mode. From openpilot log InitData
"canonical_name"	    string	Segment name
"proc_log"	            tiny_int    Log file status. See Segment File Status below	
"version"	            string	Version string from openpilot log InitData
"git_branch"	        string	Git branch from openpilot log InitData
"end_lat"	            float	Last latitude recorded in segment from GPS
"proc_camera"	        tiny_int Road camera file status. See Segment File Status below	
"canonical_route_name"	string	Route name
"devicetype"	        integer	3 is EON
"start_lat"	            float	First latitude recorded in segment from GPS
"git_dirty"	            boolean	Git dirty flag from openpilot log InitData
"url"	                string	Signed URL from which route.coords and jpegs can be downloaded (see Derived Data)
"length"	            float	Sum of distances between GPS points, miles
"dongle_id"	            string	Dongle ID
"can"	                boolean	True if log has at least 1 can message
"git_commit"	        string	Git commit from openpilot log InitData

cargo loco generate model sements canonical_name:string^ canonical_route_name:string! number:int! git_remote:string start_time_utc_millis:big_int! proc_dcamera:tiny_int! radar:bool
! create_time:int! hpgps:bool end_time_utc_millis:big_int! end_lng:float start_lng:float passive:bool proc_log:tiny_int! version:string! git_branch:string end_lat:float proc_camera:tiny_int! devicetype:tiny_int git_dirty:bool url:string! can:bool! dongle_id:string! git_commit:string

# route model
Key	Type	Description
"git_remote"	        string	Git remote from openpilot log InitData
"radar"	boolean	        True if any segment in route contains radar tracks in CAN
"create_time"	        integer	time of upload_url call for first file uploaded of route
"hpgps"	boolean	        True if any segment in route has ublox packets
"end_lng"	            float	Last longitude recorded in route, from GPS
"start_lng"	            float	First longitude recorded in route, from GPS
"passive"	            boolean	True if openpilot was running in passive mode. From openpilot log InitData
"version"	            string	Version string from openpilot log InitData
"git_branch"	        string	Git branch from openpilot log InitData
"end_lat"	            float	Last latitude recorded in route from GPS
"fullname"	            string	Route name
"devicetype"	        integer	3 is EON
"start_lat"	            float	First latitude recorded in route from GPS
"git_dirty"	            boolean	Git dirty flag from openpilot log InitData
"init_logmonotime"	    integer	Minimum logMonoTime from openpilot log
"url"	                string	Signed URL from which route.coords and jpegs can be downloaded (see Derived Data)
"length"	            float	Sum of distances between GPS points, miles
"dongle_id"	            string	Dongle ID
"can"	                boolean	True if log has at least 1 can message
"git_commit"	        string	Git commit from openpilot log InitData
"user_id"	            string	User ID of device owner
"maxlog"	            integer	Maximum log segment number uploaded
"proclog"	            integer	Maximum log segment number processed
"maxcamera"	            integer	Maximum camera segment number uploaded
"proccamera"	        integer	Maximum camera segment number processed
"maxdcamera"	        integer	Maximum front camera segment number uploaded

route has many segments
route belongs to device


