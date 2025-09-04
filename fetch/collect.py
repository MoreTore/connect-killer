import requests
import os
import time
import json
import argparse
import traceback

from database import PostgresDB
from helpers import *


def files_from_device(db, dongle_id, filters):
    upload_files = []
    upload_queue = get_upload_queue(dongle_id)
    if not isinstance(upload_queue, list) or upload_queue:  # don't request more if there are uploads queued
        return []
    print(f"Device {dongle_id} is online")
    print("getting log dir")
    log_dir = ls_log_dir(dongle_id)
    
    if log_dir:
        # Prepare a list of canonical names for the SQL IN clause
        canonical_names = [f"{dongle_id}|{file.split('/')[0]}" for file in log_dir]
        placeholders = ', '.join(['%s'] * len(canonical_names))  # Create a string with placeholders for the query

        # Execute a single query to get all relevant segments
        query = f"SELECT * FROM segments WHERE canonical_name IN ({placeholders})"
        segments = db.query_with_desc(query, canonical_names)

        # Create a dictionary for quick lookup of segments by canonical_name
        segments_dict = {segment['canonical_name']: segment for segment in segments}

        # Filter files that need to be uploaded
        for file in log_dir:
            canonical_name = f"{dongle_id}|{file.split('/')[0]}"
            segment = segments_dict.get(canonical_name)
            if not segment:
                continue

            for file_type in filters:
                if file_type in file and segment[f'{file_type}_url'] == "":
                    upload_files.append(file)
                    break
                    

    return upload_files

class File:
    def __init__(self, path):
        self.path = path
        self.type = path.split('/')[1].split('.')[0]  # Extract the file type from the path
        self.id = path.split('/')[0]  # Extract the id from the path
        self.unique_id = self.id.split('--')[1]
        self.monotonic_route_id = int(self.id.split('--')[0], 16)  # Extract the monotonic route id from the path 00000cb8 to int
        self.segment_number = int(self.id.split('--')[-1])  # Extract the segment number from the path

    def __repr__(self):
        return f"File(path={self.path}, type={self.type}, id={self.id}, segment={self.segment_number})"

def sort_upload_files(filtered_files):
    """
    Sorts the filtered files based on their type and sequence. Early qlog, rlog files are prioritized first, then
    other types in the order they appear. Only requests the next priority item if there is nothing in the higher priority to upload.
    format is like '00000cb8--a83c47336a--5/ecamera.hevc'
    where the first part is the monotonic route id, the second part is the unique identifier which is just in case, the third part is the segment number
    """
    priority = ["qlog","qcamera", "rlog", "ecamera", "fcamera", "dcamera"]
    sorted_files = {key: [] for key in priority}
    for file in filtered_files:
        file_obj = File(file)
        if file_obj.type in priority:
            sorted_files[file_obj.type].append(file_obj)
    # Sort files by monotonic_route_id
    for key in sorted_files:
        sorted_files[key].sort(key=lambda x: x.monotonic_route_id)
    # Sort each type by segment number
    for key in sorted_files:
        sorted_files[key].sort(key=lambda x: x.segment_number)
    # Only return files from the highest priority category that has files
    sorted_list = []
    for key in priority:
        if sorted_files[key]:
            sorted_list.extend(file.path for file in sorted_files[key])
            break  # Stop after finding the first priority category with files
    return sorted_list
        
def main():
    parser = argparse.ArgumentParser(description="Filter files from devices based on given substrings.")
    parser.add_argument(
        "-f", "--filters",
        nargs="+",  # Accepts multiple filters as a list
        default=["rlog", "qlog", "ecam", "fcam", "qcam"],  # Default filters if none are provided
        help="List of substrings to filter files (e.g., -f rlog ecamera)"
    )
    
    args = parser.parse_args()

    print("DATA COLLECTION PROCESS ATTEMPTING TO CONNECT TO DATABASE!")
    db = PostgresDB(
        host="localhost",
        port="5432",
        dbname=os.getenv('POSTGRES_DB', "connect_development"),
        user=os.getenv('POSTGRES_USER', "loco"),
        password=os.getenv('POSTGRES_PASSWORD', "loco")
    )
    while 1:
        db.connect()
        print("DATA COLLECTION PROCESS CONNECTED TO DATABASE!")
        devices = db.query_with_desc("SELECT dongle_id, online, firehose FROM devices;")
        uploads_dict = []
        for device in devices:
            dongle_id = device["dongle_id"]
            if device["online"] and device["firehose"]:
                log_dir = files_from_device(db, dongle_id, args.filters)
                filtered_files = list(filter(lambda file: any(sub in file for sub in args.filters), log_dir))
                filtered_files = sort_upload_files(filtered_files)
                uploads_dict.append({"dongle_id": dongle_id, "files": filtered_files})
        for upload_dict in uploads_dict:
            dongle_id = upload_dict['dongle_id']
            paths = upload_dict["files"]
            if paths:
                upload_urls = get_upload_urls(dongle_id, paths)
                request_upload(dongle_id, paths, upload_urls)
        db.close()
        print("DATA COLLECTION PROCESS CLOSED CONNECTION TO DATABASE AND GOING TO SLEEP!")
        time.sleep(600)

if __name__ == "__main__":
    print("Starting data collection script")
    while 1:
        try:
            main()
        except Exception as e:
            print("Exception occurred:")
            print(traceback.format_exc())
            time.sleep(600)
