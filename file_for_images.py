from enum import Enum

class PaymentStatus(Enum):
    COMPLETED = "completed"

def handle_charge(payment_id: str, status: PaymentStatus) -> None:
    pass

def process_webhook(payload: dict) -> None:
    ...
    event = payload.get("event")

    if event == "charge.succeeded":
        return handle_charge(payment_id=payload["id"], status=PaymentStatus.COMPLETED)

    if event == "charge.completed":
        return handle_charge(payment_id=payload["id"], status=PaymentStatus.COMPLETED)
