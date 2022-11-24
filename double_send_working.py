from websocket import create_connection
import json

ws = create_connection("ws://ENTER-ESP-IP-ADDRS/ws")

# doube sending where no reply is produced works
print("Sending WebRequest")
ws.send("Request")
ws.send("Request")
ws.close()