package handlers

import "github.com/gin-gonic/gin"

type UpdateUserRequest struct {
	Name string `json:"name"`
}

func UpdateUser(c *gin.Context) {
	var req UpdateUserRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(400, gin.H{"error": "bad request"})
		return
	}
	var user User
	db.First(&user, c.Param("id"))
	user.Name = req.Name
	db.Save(&user)
	c.JSON(200, user)
}
