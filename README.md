Issues:
- When multiple inputs hit the server in one frame, only the last one is processed on the server, 
but all are processed on the client. This leads to the client being a bit ahead of the server.
- Rollbacking doesn't work.