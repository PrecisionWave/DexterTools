#!/usr/bin/python3
import json
import sys
import zmq

context = zmq.Context()

sock = context.socket(zmq.REQ)

poller = zmq.Poller()
poller.register(sock, zmq.POLLIN)

sock.connect("tcp://127.0.0.1:5556")

# Now follows a list of commands:

detect_bank = {"command": "DetectBank"}
# This returns the JSON that deserialises to:
# {'status': 'CurrentState',
#  'state': {
#   'our_bank': 'A',
#   'desired_bank': None,
#   'our_version': '2024-06-17 13:00:09+00:00',
#   'our_extract_time': 'N/A',
#   'other_version': '2024-06-17 13:00:09+00:00',
#   'other_extract_time': None}}

# bank must be "A" or "B"
#set_desired_bank = {"command": "SetDesiredBank", "bank": "A"}
#update = {"command": "Update", "from_url": "https://bla.example/file.tar.zstd", "username": null, "password": null}
#format_other = {"command": "FormatOtherBank"}

# SetDesiredBank, Update and FormatOtherBank return either
#{'status': 'Ok', 'detail': "some string"}
# or
#{'status': 'Error', 'detail': "some string about the error"}


sock.send(json.dumps(detect_bank).encode())

socks = dict(poller.poll(1000))
if socks:
    if socks.get(sock) == zmq.POLLIN:
        data = sock.recv()
        print("Received: {}".format(data), file=sys.stderr)
        print(json.loads(data.decode()))

else:
    print("ZMQ error: timeout", file=sys.stderr)
    context.destroy(linger=5)
