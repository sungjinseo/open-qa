from rest_framework.permissions import IsAuthenticated
from rest_framework.views import APIView

class DeleteView(APIView):
    permission_classes = [IsAuthenticated]
