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

def main():
    parser = argparse.ArgumentParser(description="Filter files from devices based on given substrings.")
    parser.add_argument(
        "-f", "--filters",
        nargs="+",  # Accepts multiple filters as a list
        default=["rlog", "qlog", "ecam", "dcam", "fcam", "qcam"],  # Default filters if none are provided
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
        devices = db.query_with_desc("SELECT dongle_id, online FROM devices;")
        uploads_dict = []
        for device in devices:
            dongle_id = device["dongle_id"]
            if device["online"]:
                log_dir = files_from_device(db, dongle_id, args.filters)
                filtered_files = list(filter(lambda file: any(sub in file for sub in args.filters), log_dir))
                uploads_dict.append({"dongle_id": dongle_id, "files": filtered_files})
        for upload_dict in uploads_dict:
            dongle_id = upload_dict['dongle_id']
            paths = upload_dict["files"]
            if paths:
                upload_urls = get_upload_urls(dongle_id, paths)
                request_upload(dongle_id, paths, upload_urls)
        db.close()
        print("DATA COLLECTION PROCESS CLOSED CONNECTION TO DATABASE AND GOING TO SLEEP!")
        time.sleep(3600)

if __name__ == "__main__":
    print("Starting data collection script")
    while 1:
        try:
            main()
        except Exception as e:
            print("Exception occurred:")
            print(traceback.format_exc())
            time.sleep(3600)
