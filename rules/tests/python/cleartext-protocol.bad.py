import ftplib

def upload(host, user, pw):
    ftp = ftplib.FTP(host)
    ftp.login(user, pw)
