import sys
import os
import re
import json
import importlib.util
import traceback
import threading
import signal

# Default method timeout in seconds (can be overridden by environment variable)
DEFAULT_METHOD_TIMEOUT = int(os.getenv("PYTHON_METHOD_TIMEOUT_SECS", "8"))

# Lock to prevent mixing of stdout lines
stdout_lock = threading.Lock()

# Save original stdout for communication
__original_stdout = sys.stdout

# Redirect all future print() calls to stderr to prevent JSON corruption
sys.stdout = sys.stderr

def emit_event(event_type, data):
    """
    Sends an asynchronous event back to Exiv Kernel.
    Format: {"type": "event", "event_type": str, "data": any}
    """
    with stdout_lock:
        packet = {
            "type": "event",
            "event_type": event_type,
            "data": data
        }
        __original_stdout.write(json.dumps(packet) + "\n")
        __original_stdout.flush()

class TimeoutError(Exception):
    """Raised when method execution exceeds timeout"""
    pass

def timeout_handler(signum, frame):
    """Signal handler for SIGALRM timeout"""
    raise TimeoutError("Method execution timeout")

def call_method_with_timeout_signal(method, params, timeout_secs=8):
    """
    Call method with timeout protection using signal.SIGALRM (Unix only).

    Returns:
        dict: {"success": True, "result": <value>} on success
              {"success": False, "error": <message>} on timeout or error
    """
    # Set up timeout handler
    old_handler = signal.signal(signal.SIGALRM, timeout_handler)
    signal.alarm(timeout_secs)

    try:
        result = method(params)
        signal.alarm(0)  # Cancel alarm
        return {"success": True, "result": result}
    except TimeoutError:
        return {"success": False, "error": f"Method execution timeout ({timeout_secs} seconds)"}
    except Exception as e:
        signal.alarm(0)  # Cancel alarm on error
        return {"success": False, "error": str(e), "traceback": traceback.format_exc()}
    finally:
        signal.signal(signal.SIGALRM, old_handler)  # Restore old handler

def call_method_with_timeout_threading(method, params, timeout_secs=8):
    """
    Call method with timeout protection using threading.Timer (cross-platform).
    Less precise than signal-based approach but works on Windows.

    Returns:
        dict: {"success": True, "result": <value>} on success
              {"success": False, "error": <message>} on timeout or error
    """
    result_container = {}

    def target():
        try:
            result_container["result"] = method(params)
            result_container["success"] = True
        except Exception as e:
            result_container["success"] = False
            result_container["error"] = str(e)
            result_container["traceback"] = traceback.format_exc()

    thread = threading.Thread(target=target, daemon=True)
    thread.start()
    thread.join(timeout=timeout_secs)

    if thread.is_alive():
        # Thread still running - timeout occurred
        return {"success": False, "error": f"Method execution timeout ({timeout_secs} seconds)"}

    if result_container.get("success"):
        return {"success": True, "result": result_container["result"]}
    else:
        response = {"success": False, "error": result_container.get("error", "Unknown error")}
        if "traceback" in result_container:
            response["traceback"] = result_container["traceback"]
        return response

# Platform detection: choose appropriate timeout implementation
if sys.platform == "win32":
    call_method_with_timeout = call_method_with_timeout_threading
else:
    call_method_with_timeout = call_method_with_timeout_signal

def main():
    if len(sys.argv) < 2:
        print("Usage: bridge_runtime.py <user_script_path>", file=sys.stderr)
        sys.exit(1)

    script_path = sys.argv[1]

    # Security: Validate script path is within the allowed directory
    real_path = os.path.realpath(script_path)
    allowed_dir = os.path.realpath(os.path.dirname(__file__))
    if not real_path.startswith(allowed_dir + os.sep) and real_path != allowed_dir:
        print(f"ERROR: Script path '{script_path}' is outside allowed directory '{allowed_dir}'", file=sys.stderr)
        sys.exit(1)

    # Load user script
    try:
        spec = importlib.util.spec_from_file_location("user_logic", script_path)
        user_logic = importlib.util.module_from_spec(spec)
        # Inject emit_event into the user module
        user_logic.emit_event = emit_event
        spec.loader.exec_module(user_logic)
    except Exception as e:
        print(f"Error loading user script: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        sys.exit(1)

    # Initial setup hook if present
    if hasattr(user_logic, "setup"):
        try:
            user_logic.setup()
        except Exception as e:
            print(f"Error during setup: {e}", file=sys.stderr)

    # Allowed methods that can be called from the Rust side
    ALLOWED_METHODS = {"think", "execute", "setup", "get_manifest"}

    # Simple JSON-RPC-like loop over stdin/stdout
    for line in sys.stdin:
        if not line.strip():
            continue
        
        try:
            request = json.loads(line)
            method_name = request.get("method")
            params = request.get("params")
            request_id = request.get("id") # Keep original ID for correlation
            
            response = {"id": request_id}

            # Built-in methods
            if method_name == "get_manifest":
                manifest = getattr(user_logic, "EXIV_MANIFEST", {
                    "id": "python.unnamed",
                    "name": "Unnamed Python Script",
                    "description": "No description provided.",
                    "version": "0.0.0",
                    "capabilities": ["Reasoning"]
                })
                response["result"] = manifest
            elif method_name in ALLOWED_METHODS or (
                method_name.startswith("on_action_") and
                re.match(r'^on_action_[a-z][a-z0-9_]*$', method_name)
            ):
                if not hasattr(user_logic, method_name):
                    response["error"] = f"Method '{method_name}' not found in user script"
                else:
                    method = getattr(user_logic, method_name)
                    result_dict = call_method_with_timeout(method, params, timeout_secs=DEFAULT_METHOD_TIMEOUT)

                    if result_dict["success"]:
                        response["result"] = result_dict["result"]
                    else:
                        response["error"] = result_dict["error"]
                        if "traceback" in result_dict:
                            response["traceback"] = result_dict["traceback"]
            else:
                response["error"] = f"Method '{method_name}' not found in user script"
        except Exception as e:
            response = {"error": str(e), "traceback": traceback.format_exc()}
        
        # Write response as a single line to the ORIGINAL stdout
        with stdout_lock:
            __original_stdout.write(json.dumps(response) + "\n")
            __original_stdout.flush()

if __name__ == "__main__":
    main()