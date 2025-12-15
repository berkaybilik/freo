from enum import Enum

class PaymentStatus(Enum):
    COMPLETED = "completed"

def handle_charge(payment_id: str, status: PaymentStatus) -> None:
    pass

def process_webhook(payload: dict) -> None:
    ...
    event = payload.get("event")

    # FREO: Stripe migrated to sending `charge.completed` from `charge.succeeded` 6 months ago
    if event == "charge.completed":
        return handle_charge(payment_id=payload["id"], status=PaymentStatus.COMPLETED)
