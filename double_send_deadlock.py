# if websocket not installed -> pip3 install websocket-client

from websocket import create_connection
import json

ws = create_connection("ws://ENTER-ESP-IP-ADDRS/ws")


# double sending will deadlock
print("Sending WebRequest")
request = {"RequestWithPayload" : 42}
jsond = json.dumps(request)
ws.send(jsond)
ws.send(jsond)
print("receiving")
result = ws.recv()
print("Received '%s'" % result)
ws.close()
