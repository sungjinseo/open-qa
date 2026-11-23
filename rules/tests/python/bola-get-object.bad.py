from django.http import JsonResponse
from django.shortcuts import get_object_or_404

from .models import Order


def order_detail(request, order_id):
    order = get_object_or_404(Order, pk=order_id)
    return JsonResponse({"id": order.id, "total": order.total})
