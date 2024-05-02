import asyncio
import websockets
import json

async def send_command(ws, command):
    """Send a command to the WebSocket server."""
    await ws.send(json.dumps(command))
    print(f"Sent: {command}")

    # Wait for a response
    response = await ws.recv()
    print(f"Received: {response}")

async def test_server(uri):
    async with websockets.connect(uri) as ws:
        # Example commands for testing
        commands = [
            {"command_type": "some_command", "data": {"param1": "value1"}},
            {"command_type": "another_command", "data": {"param2": "value2"}},
        ]
        for command in commands:
            await send_command(ws, command)

if __name__ == "__main__":
    server_uri = "ws://154.38.175.6:3111/ws/v2/5325235fd"
    asyncio.run(test_server(server_uri))
