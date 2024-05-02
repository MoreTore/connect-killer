#!/usr/bin/env python3
from __future__ import annotations

import base64
import bz2
import hashlib
import io
import json
import os
import queue
import random
import select
import socket
import sys
import tempfile
import threading
import time
from dataclasses import asdict, dataclass, replace
from datetime import datetime
from functools import partial
from queue import Queue
from typing import cast
from collections.abc import Callable

import requests
from jsonrpc import JSONRPCResponseManager, dispatcher
from websocket import (ABNF, WebSocket, WebSocketException, WebSocketTimeoutException,
                       create_connection)


import jwt
import os
import requests
from datetime import datetime, timedelta

API_HOST = os.getenv('API_HOST', 'https://api.commadotai.com')

class Api():
  def __init__(self, dongle_id):
    self.dongle_id = dongle_id
    with open('/root/connect/private_key.pem', 'rb') as f:
        self.private_key = f.read()
        print(self.private_key)

  def get(self, *args, **kwargs):
    return self.request('GET', *args, **kwargs)

  def post(self, *args, **kwargs):
    return self.request('POST', *args, **kwargs)

  def request(self, method, endpoint, timeout=None, access_token=None, **params):
    return api_get(endpoint, method=method, timeout=timeout, access_token=access_token, **params)

  def get_token(self, expiry_hours=1):
    now = datetime.utcnow()
    payload = {
      'identity': self.dongle_id,
      'nbf': now,
      'iat': now,
      'exp': now + timedelta(hours=expiry_hours)
    }
    token = jwt.encode(payload, self.private_key, algorithm='RS256')
    if isinstance(token, bytes):
      token = token.decode('utf8')
    return token


def api_get(endpoint, method='GET', timeout=None, access_token=None, **params):
  headers = {}
  if access_token is not None:
    headers['Authorization'] = "JWT " + access_token

  headers['User-Agent'] = "openpilot-" + "0.9.7"

  return requests.request(method, API_HOST + "/" + endpoint, timeout=timeout, headers=headers, params=params)

ATHENA_HOST = os.getenv('ATHENA_HOST', 'ws://localhost:3111')
HANDLER_THREADS = int(os.getenv('HANDLER_THREADS', "4"))
LOCAL_PORT_WHITELIST = {8022}

LOG_ATTR_NAME = 'user.upload'
LOG_ATTR_VALUE_MAX_UNIX_TIME = int.to_bytes(2147483647, 4, sys.byteorder)
RECONNECT_TIMEOUT_S = 70

RETRY_DELAY = 10  # seconds
MAX_RETRY_COUNT = 30  # Try for at most 5 minutes if upload fails immediately
MAX_AGE = 31 * 24 * 3600  # seconds
WS_FRAME_SIZE = 4096

from typing import Union, Dict, List

UploadFileDict = Dict[str, Union[str, int, float, bool]]
UploadItemDict = Dict[str, Union[str, bool, int, float, Dict[str, str]]]

UploadFilesToUrlResponse = Dict[str, Union[int, List[UploadItemDict], List[str]]]


@dataclass
class UploadFile:
  fn: str
  url: str
  headers: dict[str, str]
  allow_cellular: bool

  @classmethod
  def from_dict(cls, d: dict) -> UploadFile:
    return cls(d.get("fn", ""), d.get("url", ""), d.get("headers", {}), d.get("allow_cellular", False))


@dataclass
class UploadItem:
  path: str
  url: str
  headers: dict[str, str]
  created_at: int
  id: str | None
  retry_count: int = 0
  current: bool = False
  progress: float = 0
  allow_cellular: bool = False

  @classmethod
  def from_dict(cls, d: dict) -> UploadItem:
    return cls(d["path"], d["url"], d["headers"], d["created_at"], d["id"], d["retry_count"], d["current"],
               d["progress"], d["allow_cellular"])


dispatcher["echo"] = lambda s: s
recv_queue: Queue[str] = queue.Queue()
send_queue: Queue[str] = queue.Queue()
upload_queue: Queue[UploadItem] = queue.Queue()
low_priority_send_queue: Queue[str] = queue.Queue()
log_recv_queue: Queue[str] = queue.Queue()
cancelled_uploads: set[str] = set()

cur_upload_items: dict[int, UploadItem | None] = {}


def strip_bz2_extension(fn: str) -> str:
  if fn.endswith('.bz2'):
    return fn[:-4]
  return fn


class AbortTransferException(Exception):
  pass


class UploadQueueCache:

  @staticmethod
  def initialize(upload_queue: Queue[UploadItem]) -> None:
    upload_queue_data = [
    {
        "path": "/root/connect/file1.txt",
        "url": "http://154.38.175.6:3111/connectincoming/164080f7933651c4/2024-03-03--06-46-42/99/rlog.bz2",
        "headers": {
            "Authorization": "Bearer sample_token1"
        },
        "created_at": 1687890123,
        "id": "upload_1",
        "retry_count": 2,
        "current": True,
        "progress": 0.5,
        "allow_cellular": True
    },
    {
        "path": "/root/connect/file2.txt",
        "url": "http://154.38.175.6:3111/connectincoming/164080f7933651c4/2024-03-03--06-46-42/89/rlog.bz2",
        "headers": {
            "Authorization": "Bearer sample_token2"
        },
        "created_at": 1687890456,
        "id": "upload_2",
        "retry_count": 0,
        "current": False,
        "progress": 0.0,
        "allow_cellular": False
    }
    ]

    # Convert the list to a JSON string
    upload_queue_json = json.dumps(upload_queue_data)

    #upload_queue_json = b'{"rlog": "/root/connect/Cargo.toml"}'#None #Params().get("AthenadUploadQueue")
    if upload_queue_json is not None:
        for item in json.loads(upload_queue_json):
            upload_queue.put(UploadItem.from_dict(item))


  @staticmethod
  def cache(upload_queue: Queue[UploadItem]) -> None:
    try:
      queue: list[UploadItem | None] = list(upload_queue.queue)
      items = [asdict(i) for i in queue if i is not None and (i.id not in cancelled_uploads)]
      #Params().put("AthenadUploadQueue", json.dumps(items))
    except Exception:
      print("athena.UploadQueueCache.cache.exception")


def handle_long_poll(ws: WebSocket, exit_event: threading.Event | None) -> None:
  end_event = threading.Event()

  threads = [
    threading.Thread(target=ws_manage, args=(ws, end_event), name='ws_manage'),
    threading.Thread(target=ws_recv, args=(ws, end_event), name='ws_recv'),
    threading.Thread(target=ws_send, args=(ws, end_event), name='ws_send'),
    threading.Thread(target=upload_handler, args=(end_event,), name='upload_handler'),
    threading.Thread(target=log_handler, args=(end_event,), name='log_handler'),
    #threading.Thread(target=stat_handler, args=(end_event,), name='stat_handler'),
  ] + [
    threading.Thread(target=jsonrpc_handler, args=(end_event,), name=f'worker_{x}')
    for x in range(HANDLER_THREADS)
  ]

  for thread in threads:
    thread.start()
  try:
    while not end_event.wait(0.1):
      if exit_event is not None and exit_event.is_set():
        end_event.set()
  except (KeyboardInterrupt, SystemExit):
    end_event.set()
    raise
  finally:
    for thread in threads:
      print(f"athena.joining {thread.name}")
      thread.join()
      
def retry_upload(tid: int, end_event: threading.Event, increase_count: bool = True) -> None:
  item = cur_upload_items[tid]
  if item is not None and item.retry_count < MAX_RETRY_COUNT:
    new_retry_count = item.retry_count + 1 if increase_count else item.retry_count

    item = replace(
      item,
      retry_count=new_retry_count,
      progress=0,
      current=False
    )
    upload_queue.put_nowait(item)
    UploadQueueCache.cache(upload_queue)

    cur_upload_items[tid] = None

    for _ in range(RETRY_DELAY):
      time.sleep(1)
      if end_event.is_set():
        break


def cb(sm, item, tid, end_event: threading.Event, sz: int, cur: int) -> None:
  # Abort transfer if connection changed to metered after starting upload
  # or if athenad is shutting down to re-connect the websocket
  sm.update(0)
  metered = sm['deviceState'].networkMetered
  if metered and (not item.allow_cellular):
    raise AbortTransferException

  if end_event.is_set():
    raise AbortTransferException

  cur_upload_items[tid] = replace(item, progress=cur / sz if sz else 1)


def upload_handler(end_event: threading.Event) -> None:
  tid = threading.get_ident()

  while not end_event.is_set():
    cur_upload_items[tid] = None

    try:
      cur_upload_items[tid] = item = replace(upload_queue.get(timeout=1), current=True)

      if item.id in cancelled_uploads:
        cancelled_uploads.remove(item.id)
        continue


      # Check if uploading over metered connection is allowed
      metered = False
      network_type = "SuperDuper"
      if metered and (not item.allow_cellular):
        retry_upload(tid, end_event, False)
        continue

      try:
        fn = item.path
        try:
          sz = os.path.getsize(fn)
        except OSError:
          sz = -1

        print(f"athena.upload_handler.upload_start: {fn}, {sz}, {item.retry_count}",)
        response = _do_upload(item)

        if response.status_code not in (200, 201, 401, 403, 412):
          print("athena.upload_handler.retry", response.status_code, fn, sz, network_type, metered)
          retry_upload(tid, end_event)
        else:
          print("athena.upload_handler.success", fn, sz)

        UploadQueueCache.cache(upload_queue)
      except (requests.exceptions.Timeout, requests.exceptions.ConnectionError, requests.exceptions.SSLError):
        print("athena.upload_handler.timeout", fn, sz, network_type, metered)
        retry_upload(tid, end_event)
      except AbortTransferException:
        print("athena.upload_handler.abort", fn, sz, network_type, metered)
        retry_upload(tid, end_event, False)

    except queue.Empty:
      pass
    except Exception:
      print("athena.upload_handler.exception")
      
def _do_upload(upload_item: UploadItem, callback: Callable = None) -> requests.Response:
  path = upload_item.path
  compress = False

  # If file does not exist, but does exist without the .bz2 extension we will compress on the fly
  if not os.path.exists(path) and os.path.exists(strip_bz2_extension(path)):
    path = strip_bz2_extension(path)
    compress = True

  with open(path, "rb") as f:
    content = f.read()
    if compress:
      print("athena.upload_handler.compress", fn=path, fn_orig=upload_item.path)
      content = bz2.compress(content)

  with io.BytesIO(content) as data:
    return requests.put(upload_item.url,
                        data=data,
                        headers={**upload_item.headers, 'Content-Length': str(len(content))},
                        timeout=30)

      
def log_handler(end_event: threading.Event) -> None:

  log_files = []
  last_scan = 0.
  while not end_event.is_set():
    try:
      curr_scan = time.monotonic()
      if curr_scan - last_scan > 10:
        log_files = ["/root/connect/164080f7933651c4_2024-03-03--06-46-42--43--rlog.bz2"]
        last_scan = curr_scan

      # send one log
      curr_log = None
      if len(log_files) > 0:
        log_entry = log_files.pop() # newest log file
        print(f"athena.log_handler.forward_request {log_entry}")
        
        curr_time = int(time.time())

        with open("/root/connect/Cargo.toml") as f:
            jsonrpc = {
                "method": "forwardLogs",
                "params": {
                "logs": f.read()
                },
                "jsonrpc": "2.0",
                "id": log_entry
            }
            low_priority_send_queue.put_nowait(json.dumps(jsonrpc))
            curr_log = log_entry


      # wait for response up to ~100 seconds
      # always read queue at least once to process any old responses that arrive
      for _ in range(100):
        if end_event.is_set():
          break
        try:
          log_resp = json.loads(log_recv_queue.get(timeout=1))
          log_entry = log_resp.get("id")
          log_success = "result" in log_resp and log_resp["result"].get("success")
          print(f"athena.log_handler.forward_response {log_entry} {log_success}")
          if curr_log == log_entry:
            break
        except queue.Empty:
          if curr_log is None:
            break

    except Exception as e:
        print(f"athena.log_handler.exception: {e}")
        
@dispatcher.add_method
def uploadFileToUrl(fn: str, url: str, headers: dict[str, str]) -> UploadFilesToUrlResponse:
  # this is because mypy doesn't understand that the decorator doesn't change the return type
  response: UploadFilesToUrlResponse = uploadFilesToUrls([{
    "fn": fn,
    "url": url,
    "headers": headers,
  }])
  return response

@dispatcher.add_method
def uploadFilesToUrls(files_data: list[UploadFileDict]) -> UploadFilesToUrlResponse:
  files = map(UploadFile.from_dict, files_data)

  items: list[UploadItemDict] = []
  failed: list[str] = []
  for file in files:
    if len(file.fn) == 0 or file.fn[0] == '/' or '..' in file.fn or len(file.url) == 0:
      failed.append(file.fn)
      continue

    path = os.path.join("", file.fn)
    if not os.path.exists(path) and not os.path.exists(strip_bz2_extension(path)):
      failed.append(file.fn)
      continue

    item = UploadItem(
      path=path,
      url=file.url,
      headers=file.headers,
      created_at=int(time.time() * 1000),
      id=None,
      allow_cellular=file.allow_cellular,
    )
    upload_id = hashlib.sha1(str(item).encode()).hexdigest()
    item = replace(item, id=upload_id)
    upload_queue.put_nowait(item)
    items.append(asdict(item))

  UploadQueueCache.cache(upload_queue)

  resp: UploadFilesToUrlResponse = {"enqueued": len(items), "items": items}
  if failed:
    resp["failed"] = failed

  return resp
     
def startLocalProxy(global_end_event: threading.Event, remote_ws_uri: str, local_port: int) -> dict[str, int]:
  try:
    if local_port not in LOCAL_PORT_WHITELIST:
      raise Exception("Requested local port not whitelisted")

    print("athena.startLocalProxy.starting")

    dongle_id = "164080f7933651c4"
    identity_token = Api(dongle_id).get_token()
    ws = create_connection(remote_ws_uri,
                           cookie="jwt=" + identity_token,
                           enable_multithread=True)

    ssock, csock = socket.socketpair()
    local_sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    local_sock.connect(('127.0.0.1', local_port))
    local_sock.setblocking(False)

    proxy_end_event = threading.Event()
    threads = [
      threading.Thread(target=ws_proxy_recv, args=(ws, local_sock, ssock, proxy_end_event, global_end_event)),
      threading.Thread(target=ws_proxy_send, args=(ws, local_sock, csock, proxy_end_event))
    ]
    for thread in threads:
      thread.start()

    print("athena.startLocalProxy.started")
    return {"success": 1}
  except Exception as e:
    print("athenad.startLocalProxy.exception")
    raise e


def jsonrpc_handler(end_event: threading.Event) -> None:
  dispatcher["startLocalProxy"] = partial(startLocalProxy, end_event)
  while not end_event.is_set():
    try:
      data = recv_queue.get(timeout=1)
      if "method" in data:
        print(f"athena.jsonrpc_handler.call_method {data=}")
        response = JSONRPCResponseManager.handle(data, dispatcher)
        send_queue.put_nowait(response.json)
      elif "id" in data and ("result" in data or "error" in data):
        log_recv_queue.put_nowait(data)
      else:
        raise Exception("not a valid request or response")
    except queue.Empty:
      pass
    except Exception as e:
      print("athena jsonrpc handler failed")
      send_queue.put_nowait(json.dumps({"error": str(e)}))

def ws_proxy_recv(ws: WebSocket, local_sock: socket.socket, ssock: socket.socket, end_event: threading.Event, global_end_event: threading.Event) -> None:
  while not (end_event.is_set() or global_end_event.is_set()):
    try:
      data = ws.recv()
      if isinstance(data, str):
        data = data.encode("utf-8")
      local_sock.sendall(data)
    except WebSocketTimeoutException:
      pass
    except Exception:
      print("athenad.ws_proxy_recv.exception")
      break

  print("athena.ws_proxy_recv closing sockets")
  ssock.close()
  local_sock.close()
  print("athena.ws_proxy_recv done closing sockets")

  end_event.set()

@dispatcher.add_method
def getPublicKey() -> str | None:
    with open('/root/connect/private_key.pem') as f:
        return f.read()

def ws_proxy_send(ws: WebSocket, local_sock: socket.socket, signal_sock: socket.socket, end_event: threading.Event) -> None:
  while not end_event.is_set():
    try:
      r, _, _ = select.select((local_sock, signal_sock), (), ())
      if r:
        if r[0].fileno() == signal_sock.fileno():
          # got end signal from ws_proxy_recv
          end_event.set()
          break
        data = local_sock.recv(4096)
        if not data:
          # local_sock is dead
          end_event.set()
          break

        ws.send(data, ABNF.OPCODE_BINARY)
    except Exception:
      print("athenad.ws_proxy_send.exception")
      end_event.set()

  print("athena.ws_proxy_send closing sockets")
  signal_sock.close()
  print("athena.ws_proxy_send done closing sockets")


def ws_recv(ws: WebSocket, end_event: threading.Event) -> None:
  last_ping = int(time.monotonic() * 1e9)
  while not end_event.is_set():
    try:
      opcode, data = ws.recv_data(control_frame=True)
      if opcode in (ABNF.OPCODE_TEXT, ABNF.OPCODE_BINARY):
        if opcode == ABNF.OPCODE_TEXT:
          data = data.decode("utf-8")
        recv_queue.put_nowait(data)
      elif opcode == ABNF.OPCODE_PING:
        last_ping = int(time.monotonic() * 1e9)
        #Params().put("LastAthenaPingTime", str(last_ping))
    except WebSocketTimeoutException:
      ns_since_last_ping = int(time.monotonic() * 1e9) - last_ping
      if ns_since_last_ping > RECONNECT_TIMEOUT_S * 1e9:
        print("athenad.ws_recv.timeout")
        end_event.set()
    except Exception:
      print("athenad.ws_recv.exception")
      end_event.set()


def ws_send(ws: WebSocket, end_event: threading.Event) -> None:
  while not end_event.is_set():
    try:
      try:
        data = send_queue.get_nowait()
      except queue.Empty:
        data = low_priority_send_queue.get(timeout=1)
      for i in range(0, len(data), WS_FRAME_SIZE):
        frame = data[i:i+WS_FRAME_SIZE]
        last = i + WS_FRAME_SIZE >= len(data)
        opcode = ABNF.OPCODE_TEXT if i == 0 else ABNF.OPCODE_CONT
        ws.send_frame(ABNF.create_frame(frame, opcode, last))
    except queue.Empty:
      pass
    except Exception:
      print("athenad.ws_send.exception")
      end_event.set()


def ws_manage(ws: WebSocket, end_event: threading.Event) -> None:
  #params = Params()
  onroad_prev = None
  sock = ws.sock

  while True:
    onroad = False#params.get_bool("IsOnroad")
    if onroad != onroad_prev:
      onroad_prev = onroad

      if sock is not None:
        # While not sending data, onroad, we can expect to time out in 7 + (7 * 2) = 21s
        #                         offroad, we can expect to time out in 30 + (10 * 3) = 60s
        # FIXME: TCP_USER_TIMEOUT is effectively 2x for some reason (32s), so it's mostly unused
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_USER_TIMEOUT, 16000 if onroad else 0)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPIDLE, 7 if onroad else 30)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPINTVL, 7 if onroad else 10)
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_KEEPCNT, 2 if onroad else 3)

    if end_event.wait(5):
      break


def backoff(retries: int) -> int:
  return random.randrange(0, min(128, int(2 ** retries)))


def main(exit_event: threading.Event = None):

  dongle_id = "164080f7933651c4" #params.get("DongleId", encoding='utf-8')
  UploadQueueCache.initialize(upload_queue)

  ws_uri = ATHENA_HOST + "/ws/v2/" + dongle_id
  api = Api(dongle_id)
  print(api.get_token())

  conn_start = None
  conn_retries = 0
  while exit_event is None or not exit_event.is_set():
    try:
      if conn_start is None:
        conn_start = time.monotonic()

      print(f"athenad.main.connecting_ws {ws_uri=} {conn_retries=}")
      ws = create_connection(ws_uri,
                             cookie="jwt=" + api.get_token(),
                             enable_multithread=True,
                             timeout=30.0)
      print(f"athenad.main.connecting_ws {ws_uri=} {conn_retries=} {time.monotonic() - conn_start=}")
      conn_start = None

      conn_retries = 0
      cur_upload_items.clear()

      handle_long_poll(ws, exit_event)
    except (KeyboardInterrupt, SystemExit):
      break
    except (ConnectionError, TimeoutError, WebSocketException):
      conn_retries += 1
      #params.remove("LastAthenaPingTime")
    except Exception as e:
      print(f"athenad.main.exception: {e}")

      conn_retries += 1
      #params.remove("LastAthenaPingTime")

    time.sleep(backoff(conn_retries))


if __name__ == "__main__":
  main()