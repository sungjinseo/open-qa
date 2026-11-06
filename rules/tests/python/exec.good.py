HANDLERS = {"start": lambda: "started", "stop": lambda: "stopped"}

def run(command):
    return HANDLERS[command]()
