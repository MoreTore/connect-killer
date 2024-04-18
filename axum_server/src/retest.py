import re

def transform_string(input_string):
    pattern = r"^([a-z0-9]{16})_([0-9]{4}-[0-9]{2}-[0-9]{2})--([0-9]{2}-[0-9]{2}-[0-9]{2})--([0-9]+)--(.+)$"
    match = re.match(pattern, input_string)

    if match:
        parts = match.groups()
        transformed = f"{parts[0]}/{parts[1]}--{parts[2]}/{parts[3]}/{parts[4]}"
        return transformed
    else:
        return "No match found"

# Example usage
input_string = "164080f7933651c4_2024-03-02--19-02-46--0--rlog.bz2"
transformed_string = transform_string(input_string)
print(transformed_string)
