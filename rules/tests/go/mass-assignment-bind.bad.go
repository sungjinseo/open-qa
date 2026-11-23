package handlers

import "github.com/gin-gonic/gin"

func UpdateUser(c *gin.Context) {
	var user User
	if err := c.ShouldBindJSON(&user); err != nil {
		c.JSON(400, gin.H{"error": "bad request"})
		return
	}
	db.Save(&user)
	c.JSON(200, user)
}
