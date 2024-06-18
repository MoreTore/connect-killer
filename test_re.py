import re

def transform_route_string(input_string: str) -> str:
    # Regex for the old format
    re_drive_log = re.compile(r"^([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)(?:--|/)(.+)$")
    # Regex for the new format
    re_new_format = re.compile(r"^([0-9a-f]{8})--([0-9a-f]{10})--([0-9]+)/(.+)$")
    re_crash_format = re.compile(r"^crash/([0-9a-f]{8}--[0-9a-f]{10}|[0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2})_([0-9a-f]{8})_(.+)$")
    re_boot_log = re.compile(r"^boot/([0-9a-f]{8}--[0-9a-f]{10}|[0-9]{4}-[0-9]{2}-[0-9]{2}--[0-9]{2}-[0-9]{2}-[0-9]{2}).bz2$")
    if re_drive_log.match(input_string):
        caps = re_drive_log.match(input_string)
        return f"{caps[1]}--{caps[2]}/{caps[3]}/{caps[4]}"
    elif re_new_format.match(input_string):
        caps = re_new_format.match(input_string)
        return f"{caps[1]}--{caps[2]}/{caps[3]}/{caps[4]}"
    elif re_crash_format.match(input_string):
        caps = re_crash_format.match(input_string)
        return f"crash/{caps[1]}/{caps[2]}/{caps[3]}"
    elif re_boot_log.match(input_string):
        return input_string
    else:
        return "Invalid"

# Test cases
test_inputs = [
    "2024-03-02--19-02-46--0--rlog.bz2",
    # "2024-03-02--19-02-46--0/rlog",
    "0000008c--8a84371aea--0/rlog.bz2",
    # "0000008c--8a84371aea--0/qcam.ts",
    # "0000008c--8a84371aea--0/qcam.hevc",
    "boot/0000008c--8a84371aea.bz2",
    # "boot/2024-06-01--02-32-09.bz2"
    "crash/0000008c--8a84371aea_88888888_crashname",
    "crash/2024-03-02--19-02-46_88888888__crashname"
]

test_outputs = [transform_route_string(input_str) for input_str in test_inputs]
print(test_outputs)
